//! The [`Runtime`]: every live port behind one `Clone`-able bag, plus a
//! convenience over the application layer's transaction orchestrator.

use std::sync::Arc;

use adapter_pg::PgPool;
use base64::Engine as _;
use domain::ports::{
    AccountStore, Authenticator, ChangelogStore, ColumnStore, CommissionStore, Database, DidMinter,
    FileStore, ProfileCache, ProfileSource, UnitOfWorkFn, UserStore, WorkflowStore,
};
use fluent_uri::Uri;

use crate::{Config, ensure_custody_hardened};

/// The composition root's bag of dependencies — every live port behind an
/// `Arc<dyn Trait>`, so the live adapter is picked once here and drivers stay
/// ignorant of it. `Clone` is cheap, so each request may hold its own copy.
#[derive(Clone)]
pub struct Runtime {
    /// The resolved runtime [`Config`], kept whole.
    pub config: Config,
    /// The Postgres connection pool, shared directly because the `health` probe
    /// reads it as well as the adapters built over it.
    pub pool: PgPool,
    /// The OAuth handshake with a visitor's PDS: `start` yields the
    /// authorization URL, `complete` exchanges the callback for a DID.
    pub auth: Arc<dyn Authenticator>,
    /// User reads by id or DID. `provision` is a write and lives on the
    /// [`UnitOfWork`](domain::ports::UnitOfWork).
    pub users: Arc<dyn UserStore>,
    /// Public profiles read from the PDS. A failure degrades `me` to the bare
    /// DID rather than erroring.
    pub profile_source: Arc<dyn ProfileSource>,
    /// Read-through cache fronting [`profile_source`](Runtime::profile_source).
    /// Pool-backed both ways — a documented exception to the unit-of-work
    /// write rule, since the fill carries no transactional invariant.
    pub profile_cache: Arc<dyn ProfileCache>,
    /// Account, membership and invitation reads; every account write lives on
    /// the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub accounts: Arc<dyn AccountStore>,
    /// The canonical commission reads, including the `is_participant` predicate
    /// every commission act authorizes through.
    pub commissions: Arc<dyn CommissionStore>,
    /// The ordered, participant-only changelog read. The append is a
    /// [`UnitOfWork`](domain::ports::UnitOfWork) view, so entries commit
    /// atomically with the writes they record.
    pub changelog: Arc<dyn ChangelogStore>,
    /// An account's boards and where each card sits on them. Mutations are a
    /// [`UnitOfWork`](domain::ports::UnitOfWork) view, since moving a card
    /// rewrites the neighbours it displaces.
    pub workflows: Arc<dyn WorkflowStore>,
    /// A board's columns. Separate from [`workflows`](Runtime::workflows)
    /// because a column carries its own id and visibility; their order lives on
    /// the workflow.
    pub columns: Arc<dyn ColumnStore>,
    /// The private blob store behind a commission file entry. Pool-backed and
    /// outside the unit of work — bytes cannot ride a transaction, so an
    /// orphan on rollback is accepted.
    pub files: Arc<dyn FileStore>,
    /// The write factory: the only way to reach a private-store domain write.
    /// A caller `begin()`s, writes through the returned
    /// [`UnitOfWork`](domain::ports::UnitOfWork)'s views, then `commit()`s once
    /// — drop rolls back.
    pub database: Arc<dyn Database>,
    /// Mints a sovereign `did:plc` for a newly founded account.
    pub did_minter: Arc<dyn DidMinter>,
}

impl Runtime {
    /// The [`application::App`] orchestrator over this runtime's adapters.
    /// Drivers build it once at boot and call use cases through it.
    pub fn app(&self) -> application::App {
        application::App::from(self)
    }

    /// Run `f` inside one private-store transaction. Delegates to
    /// [`application::transaction`] over [`database`](Runtime::database).
    pub async fn transaction<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: for<'a> UnitOfWorkFn<'a, T> + Send,
        T: Send,
    {
        application::transaction(&*self.database, f).await
    }
}

impl Runtime {
    /// Wire the live adapters over `config`: a Postgres pool (connected here —
    /// migrations are NOT run, the caller calls [`adapter_pg::migrate`]), the
    /// atproto [`Authenticator`], the `did:plc` custody chain, and every
    /// pg-backed store.
    ///
    /// Fails if the pool cannot connect, `public_url` or the root key will not
    /// parse, or [`ensure_custody_hardened`] refuses the configuration; the
    /// error messages never echo the secrets themselves.
    pub async fn connect(config: Config) -> Result<Self, ConnectError> {
        let pool = adapter_pg::connect(&config.database_url)
            .await
            .map_err(ConnectError::Database)?;
        tracing::info!("database pool established");
        Self::wire(config, pool).map_err(ConnectError::Setup)
    }

    /// The pure-construction half of [`connect`](Runtime::connect), over an
    /// already-connected pool.
    fn wire(config: Config, pool: PgPool) -> anyhow::Result<Self> {
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
            workflows: Arc::new(adapter_pg::PgWorkflowStore::new(pool.clone())),
            columns: Arc::new(adapter_pg::PgColumnStore::new(pool.clone())),
            files: Arc::new(adapter_pg::PgFileStore::new(pool.clone())),
            database: Arc::new(adapter_pg::PgDatabase::new(pool.clone())),
            pool,
        };
        Ok(runtime)
    }
}

/// Why [`Runtime::connect`] refused to boot: split so a driver can tell an
/// unreachable database from a misconfiguration.
#[derive(Debug)]
pub enum ConnectError {
    /// The Postgres pool could not connect to [`Config::database_url`].
    Database(adapter_pg::SqlxError),
    /// The configuration is unusable: bad `public_url` or root key, or the
    /// custody guard refused it.
    Setup(anyhow::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Database(e) => write!(f, "database unreachable: {e}"),
            ConnectError::Setup(e) => write!(f, "runtime setup failed: {e:#}"),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConnectError::Database(e) => Some(e),
            ConnectError::Setup(e) => Some(e.as_ref()),
        }
    }
}
