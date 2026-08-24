//! The [`Runtime`]: every live port behind one `Clone`-able bag, plus the one
//! transaction orchestrator every driver shares.
//!
//! Moved from `api::AppState` (ZMVP-200). `api` re-exports it as `AppState`
//! for its handlers; `cli` drives it directly.

use std::sync::Arc;

use adapter_pg::PgPool;
use base64::Engine as _;
use domain::ports::{
    AccountStore, Authenticator, ChangelogStore, CommissionStore, Database, DidMinter, FileStore,
    ProfileCache, ProfileSource, UnitOfWorkFn, UserStore,
};
use fluent_uri::Uri;

use crate::{Config, ensure_custody_hardened};

/// The composition root's bag of dependencies — every live port behind an
/// `Arc<dyn Trait>`. `api` hands it to every handler via axum's `State`
/// extractor (re-exported there as `AppState`); `cli` holds one per process. It is `Clone` (the
/// pool and every port are cheaply cloneable, behind [`PgPool`]/[`Arc`]), so axum
/// can hand each request its own copy.
///
/// Each port is an `Arc<dyn Trait>` precisely so the wiring picks the live
/// adapter once, in `main`, and the handlers stay ignorant of it: pg/atproto in
/// production, the in-process fakes (mem + a fake PDS) in the e2e tests. Adding a
/// capability is adding a field here plus a line in `main` — never a handler
/// rewrite.
///
/// References: DESIGN "Domains and Applications"; [`Runtime::connect`].
#[derive(Clone)]
pub struct Runtime {
    /// The resolved runtime [`Config`]. Kept whole so handlers and `main` read
    /// the same values (e.g. cookie security keys off [`Config::env`]).
    pub config: Config,
    /// The Postgres connection pool. Shared directly (not behind a port) because
    /// it backs both the adapters built over it and the `health` probe.
    pub pool: PgPool,
    /// The [`Authenticator`] port: drives the OAuth handshake with a visitor's
    /// PDS — `start` yields the authorization URL, `complete` exchanges the
    /// callback for a DID. A trait object so the composition root chooses the
    /// live adapter (atproto's `AtprotoAuthenticator` in `main`, a fake PDS in
    /// e2e tests). Used by the `signin` and `signin_callback` handlers.
    pub auth: Arc<dyn Authenticator>,
    /// The [`UserStore`] read port: resolves a recognized visitor by id
    /// (`find`, the session-resolution path) or DID (`find_by_did`), off the pool.
    /// *Recognition* (`provision`) is a write and lives on the
    /// [`UnitOfWork`](domain::ports::UnitOfWork) vended by [`database`](Runtime::database).
    /// pg in `main`, mem in tests.
    pub users: Arc<dyn UserStore>,
    /// The [`ProfileSource`] port: reads public profiles from the PDS. atproto
    /// in `main`, a fake in tests. A failure here degrades the `me` page to the
    /// DID rather than erroring.
    pub profile_source: Arc<dyn ProfileSource>,
    /// The [`ProfileCache`] port: private read-through cache fronting
    /// [`profile_source`](Runtime::profile_source). Both `get` and the best-effort
    /// `put` are pool-backed — the cache fill is a documented exception to the Unit
    /// of Work (a read-path write with no transactional invariant; DD `24150017`).
    /// pg in `main` (entries expire after an hour, set in `main`), mem in tests.
    /// See `resolve_profile`.
    pub profile_cache: Arc<dyn ProfileCache>,
    /// The [`AccountStore`] read port: account/membership/invitation reads
    /// (`find`, `role_of`, `find_pending_invitation`, `find_invitation`) off the
    /// pool. Every account *write* lives on the [`UnitOfWork`](domain::ports::UnitOfWork)
    /// vended by [`database`](Runtime::database). pg in `main`, mem in tests.
    pub accounts: Arc<dyn AccountStore>,
    /// The [`CommissionStore`] read port (ZMVP-87): the canonical commission
    /// reads — `find`, and the `is_participant` predicate every "a Participant
    /// does X" endpoint authorizes through (owner-arm-only until ZMVP-79 adds
    /// the seated arm). Commission *writes* live on the
    /// [`UnitOfWork`](domain::ports::UnitOfWork) vended by
    /// [`database`](Runtime::database). pg in `main`, mem in tests.
    pub commissions: Arc<dyn CommissionStore>,
    /// The [`ChangelogStore`] read port (ZMVP-87): the ordered, participant-only
    /// changelog read. The *append* is a [`UnitOfWork`](domain::ports::UnitOfWork)
    /// view (`uow.changelog()`) — entries commit atomically with the domain
    /// writes they record (Changelog DD D4). pg in `main`, mem in tests.
    pub changelog: Arc<dyn ChangelogStore>,
    /// The [`FileStore`] port (ZMVP-88): the private blob store behind a commission
    /// file entry. Pool-backed and **outside** the Unit of Work — the blob write is
    /// a step that precedes the unit recording the file entry (bytes cannot ride a
    /// transaction; orphan-on-rollback accepted). v1 ships a mock/local
    /// implementation (a pg `bytea` table in `main`, the in-memory fake in tests);
    /// the real blob architecture is the future blob-architecture walkthrough.
    pub files: Arc<dyn FileStore>,
    /// The [`Database`] write factory: the **only** way to reach a private-store
    /// domain write. A handler calls `begin()`, issues its writes through the
    /// returned [`UnitOfWork`](domain::ports::UnitOfWork)'s view accessors
    /// (`uow.accounts().create(...)`, `uow.users().provision(...)`), then
    /// `commit()`s once (drop = rollback). Such writes cannot skip a transaction by
    /// construction (DD `24150017`). The profile cache is a documented exception —
    /// its best-effort fill is pool-backed (see [`profile_cache`](Runtime::profile_cache)).
    /// pg in `main`, mem in tests.
    pub database: Arc<dyn Database>,
    /// The [`DidMinter`] port: mints a sovereign `did:plc` for a newly founded
    /// account. The live adapter is `RealDidMinter` (generates the account's
    /// rotation keys, signs an identity-only genesis operation, custodies the keys
    /// via `PgKeyStore`, and submits to a — no-op in v1 — directory); the mem/stub
    /// minter is used in tests. Used by the `create_account` handler.
    pub did_minter: Arc<dyn DidMinter>,
}

/// Run `f` inside one private-store transaction — the **one** place in this
/// crate that orchestrates `begin`/`commit`/`rollback` (DD "Transactions as a
/// capability" `24150017`). Opens a [`UnitOfWork`](domain::ports::UnitOfWork) via
/// [`Database::begin`](domain::ports::Database::begin), hands it to `f` as
/// `&mut dyn UnitOfWork`, then **commits on `Ok`, rolls back on `Err`**: the
/// closure body *is* the transaction boundary, so a commit can never be
/// forgotten. Strictly intra-Postgres; never a cross-store dual write.
///
/// `pub`, taking a bare `&dyn Database`, so both
/// [`Runtime::transaction`] (every route) and `api`'s deadline sweeper (which
/// holds a `Database` handle but no [`Runtime`]) share
/// this one orchestrator rather than each re-implementing commit/rollback
/// (ZMVP-111; Engineer ruling on PR #100). `f`'s bound is
/// [`UnitOfWorkFn`](domain::ports::UnitOfWorkFn) plus explicit `F: Send`/`T: Send`
/// (the returned future holds both across `.await`s, so `Fut: Send` alone would
/// not keep a handler future `Send`), not std's `AsyncFnOnce` — see
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

impl Runtime {
    /// Run `f` inside one private-store transaction — the **only** way a route
    /// reaches a private-store write. Delegates to the crate-level
    /// [`transaction`] orchestrator over [`self.database`](Runtime::database).
    ///
    /// The call site reads `state.transaction(async |uow: &mut dyn UnitOfWork| {
    /// … }).await?` — an `async` closure, no `Box::pin`, no `&*state.database`.
    pub async fn transaction<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: for<'a> UnitOfWorkFn<'a, T> + Send,
        T: Send,
    {
        transaction(&*self.database, f).await
    }
}

impl Runtime {
    /// Wire the **live** adapters over `config` — the one production
    /// composition, shared by every driver: a Postgres pool from
    /// [`Config::database_url`] (connected here, **migrations not run** — the
    /// caller decides, explicitly, via [`adapter_pg::migrate`]), the atproto
    /// [`Authenticator`] with its redirect URI built from
    /// [`Config::public_url`], the `did:plc` custody chain (root key decoded from
    /// [`Config::did_key_root_key`], [`ensure_custody_hardened`] enforced,
    /// `PgKeyStore` + operation log + directory → `RealDidMinter`), and every
    /// pg-backed store.
    ///
    /// Caveats: fails — and the driver must not run — if the pool cannot
    /// connect, `public_url` is not a parseable URI, the root key is not
    /// base64, or the custody guard refuses the configuration. The error
    /// messages never echo the secrets themselves.
    ///
    /// References: DESIGN "Domains and Applications"; ZMVP-200.
    pub async fn connect(config: Config) -> anyhow::Result<Self> {
        let pool = adapter_pg::connect(&config.database_url).await?;
        tracing::info!("database pool established");

        let redirect_uri =
            Uri::parse(format!("{}/signin-callback", config.public_url)).map_err(|(e, uri)| {
                anyhow::anyhow!("invalid public_url, cannot build redirect URI ({uri}): {e}")
            })?;

        let root_key_bytes = base64::engine::general_purpose::STANDARD
            .decode(config.did_key_root_key.trim())
            .map_err(|e| anyhow::anyhow!("ZURFUR_DID_KEY_ROOT_KEY must be valid base64: {e}"))?;
        ensure_custody_hardened(&config.env, &root_key_bytes, config.plc_directory_submit)?;
        let root_key = adapter_pg::RootKey::from_bytes(&root_key_bytes)?;
        let key_store = Arc::new(adapter_pg::PgKeyStore::new(pool.clone(), root_key));
        let oauth_vault = adapter_atproto::SecretVault::from_bytes(&root_key_bytes)?;
        let op_log = Arc::new(adapter_pg::PgPlcOperationLog::new(pool.clone()));
        let directory =
            adapter_atproto::plc_directory_from_config(&adapter_atproto::DirectoryConfig {
                endpoint: config.plc_directory_endpoint.clone(),
                enabled: config.plc_directory_submit,
            });
        let did_minter = Arc::new(adapter_atproto::RealDidMinter::new(
            key_store, op_log, directory,
        ));

        let runtime = Runtime {
            config,
            auth: Arc::new(adapter_atproto::AtprotoAuthenticator::new(
                redirect_uri,
                pool.clone(),
                oauth_vault,
            )),
            users: Arc::new(adapter_pg::PgUserStore::new(pool.clone())),
            profile_source: Arc::new(adapter_atproto::AtprotoProfileSource::new()),
            profile_cache: Arc::new(adapter_pg::PgProfileCache::new(
                pool.clone(),
                std::time::Duration::from_secs(60 * 60),
            )),
            did_minter,
            accounts: Arc::new(adapter_pg::PgAccountStore::new(pool.clone())),
            commissions: Arc::new(adapter_pg::PgCommissionStore::new(pool.clone())),
            changelog: Arc::new(adapter_pg::PgChangelogStore::new(pool.clone())),
            files: Arc::new(adapter_pg::PgFileStore::new(pool.clone())),
            database: Arc::new(adapter_pg::PgDatabase::new(pool.clone())),
            pool,
        };
        Ok(runtime)
    }
}
