//! The one `begin`/`commit`/`rollback` orchestrator (DD "Transactions as a
//! capability" `24150017`). Lives here because the use case owns its
//! transaction boundary: what is atomic is a domain rule, not a transport
//! detail. Moved from `composition` (ZMVP-205); `Runtime::transaction` there
//! still delegates to it for the drivers' not-yet-migrated call sites.

use domain::ports::{Database, UnitOfWorkFn};

/// Run `f` inside one private-store transaction. Opens a
/// [`UnitOfWork`](domain::ports::UnitOfWork) via
/// [`Database::begin`], hands it to `f` as `&mut dyn UnitOfWork`, then
/// **commits on `Ok`, rolls back on `Err`**: the closure body *is* the
/// transaction boundary, so a commit can never be forgotten. Strictly
/// intra-Postgres; never a cross-store dual write.
///
/// Takes a bare `&dyn Database` so every use case, `composition`'s
/// `Runtime::transaction`, and `api`'s deadline sweeper (which holds a
/// `Database` handle but no runtime) share this one orchestrator rather than
/// each re-implementing commit/rollback (ZMVP-111; Engineer ruling on PR
/// #100). `f`'s bound is [`UnitOfWorkFn`] plus explicit `F: Send`/`T: Send`
/// (the returned future holds both across `.await`s, so `Fut: Send` alone
/// would not keep a handler future `Send`), not std's `AsyncFnOnce` — see
/// that trait's doc comment for why (a compiler limitation with higher-ranked
/// `AsyncFnOnce` bounds, rust-lang/rust#110338).
pub async fn transaction<T, F>(db: &dyn Database, f: F) -> anyhow::Result<T>
where
    F: for<'a> UnitOfWorkFn<'a, T> + Send,
    T: Send,
{
    let mut uow = db.begin().await?;
    match f(&mut *uow).await {
        Ok(value) => {
            uow.commit().await?;
            Ok(value)
        }
        Err(err) => {
            // The closure's error is the meaningful one (e.g. `HandleTaken` →
            // 409); a rollback failure must never replace it. The unit is
            // abandoned either way (an uncommitted transaction also rolls back
            // on drop), so a rollback error here is secondary and deliberately
            // not surfaced over `err`.
            let _ = uow.rollback().await;
            Err(err)
        }
    }
}
