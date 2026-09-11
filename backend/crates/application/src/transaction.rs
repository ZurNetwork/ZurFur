//! The one `begin`/`commit`/`rollback` orchestrator for the private store
//! (DD 24150017). The use case owns its transaction boundary.

use domain::ports::{Database, UnitOfWorkFn};

/// Run `f` inside one private-store transaction: opens a
/// [`UnitOfWork`](domain::ports::UnitOfWork) via [`Database::begin`], hands it
/// to `f`, then commits on `Ok` and rolls back on `Err`. Strictly
/// intra-Postgres; never a cross-store dual write.
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
            // The closure's error is the meaningful one; a rollback failure
            // must never replace it (drop rolls back either way).
            let _ = uow.rollback().await;
            Err(err)
        }
    }
}
