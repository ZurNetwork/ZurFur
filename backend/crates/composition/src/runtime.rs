//! The [`Runtime`]: every live port behind one `Clone`-able bag, plus a
//! convenience over the application layer's transaction orchestrator.

use std::future::Future;
use std::sync::Arc;

use adapter_pg::{CustodyMigrator, PgPool};
use domain::elements::custody::Purpose;
use domain::ports::character::CharacterStore;
use domain::ports::{
    AccountStore, Authenticator, BugReportStore, ChangelogStore, ColumnStore, CommissionStore,
    Database, DidMinter, FileStore, IdentityResolver, ProfileCache, ProfileSource, ScopedMac,
    UnitOfWorkFn, UserStore, WorkflowStore,
};
use fluent_uri::Uri;

use crate::custody::{Custody, derive_keeping_legacy_root};
use crate::{Config, RootSecret};

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
    /// Handle ↔ DID lookups for actors Zurfur does not own, confirmed both
    /// ways. The one resolver behind sign-in and the profile read as well.
    pub identity_resolver: Arc<dyn IdentityResolver>,
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
    /// The profile lookups running behind `me` and the Den's root name, one
    /// per DID, shared by every clone of this runtime so concurrent misses
    /// start one fetch.
    pub profile_lookups: application::user::ProfileLookups,
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
    /// Reads arrive with the Character slice; the port declares none yet.
    pub characters: Arc<dyn CharacterStore>,
    /// The bug reports Users file: the per-reporter count behind the rate
    /// limit, and the list. Filing is a [`UnitOfWork`](domain::ports::UnitOfWork)
    /// view.
    pub bug_reports: Arc<dyn BugReportStore>,
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
    /// The Den's MAC handle: tags and verifies the Den's records under the Den's key.
    /// Answers `PurposeNotHeld` in a runtime connected without that purpose.
    pub den_mac: ScopedMac,
}

/// A runtime crosses threads and is cloned per request; a field that can't (the custody
/// root) fails to compile here.
const _: () = {
    const fn crosses_threads_and_clones<T: Send + Sync + Clone>() {}
    crosses_threads_and_clones::<Runtime>();
};

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
    /// Wire every live adapter over `config` and `custody`'s handles, on a Postgres pool
    /// connected here and never migrated: composition builds the [`CustodyMigrator`], the
    /// driver runs it. Fails if the pool can't connect, `public_url` won't parse, or the
    /// system DNS configuration can't be read.
    pub async fn connect(config: Config, custody: Custody) -> Result<Self, ConnectError> {
        let pool = adapter_pg::connect(&config.database_url)
            .await
            .map_err(ConnectError::Database)?;
        tracing::info!("database pool established");
        Self::wire(config, pool, custody).map_err(ConnectError::Setup)
    }

    /// [`connect`](Runtime::connect) with `purposes` derived from `root`, plus the
    /// [`CustodyMigrator`] the driver runs. The root is guarded and consumed at the call;
    /// a zeroizing copy of its bytes rides the returned future into the migrator, which
    /// keeps it until `run` returns or the migrator is dropped.
    pub fn connect_migrating(
        config: Config,
        root: RootSecret,
        purposes: &[Purpose],
    ) -> impl Future<Output = Result<(Self, CustodyMigrator), ConnectError>> + Send + 'static {
        let derived =
            derive_keeping_legacy_root(root, &config.env, config.plc_directory_submit, purposes);
        async move {
            let (custody, legacy_root) = derived.map_err(ConnectError::Setup)?;
            let migrator_sealer = custody.did_custody.clone();
            let runtime = Self::connect(config, custody).await?;
            let custody_migrator =
                CustodyMigrator::new(runtime.pool.clone(), legacy_root, migrator_sealer);
            Ok((runtime, custody_migrator))
        }
    }

    /// The pure-construction half of [`connect`](Runtime::connect), over an
    /// already-connected pool.
    fn wire(config: Config, pool: PgPool, custody: Custody) -> anyhow::Result<Self> {
        let public_url: &url::Url = config.public_url.as_ref();
        let callback = public_url
            .join("/signin-callback")
            .map_err(|e| anyhow::anyhow!("invalid public_url, cannot build redirect URI: {e}"))?;
        let redirect_uri = Uri::parse(String::from(callback)).map_err(|(e, uri)| {
            anyhow::anyhow!("invalid public_url, cannot build redirect URI ({uri}): {e}")
        })?;

        let key_store = Arc::new(adapter_pg::PgKeyStore::new(
            pool.clone(),
            custody.did_custody,
        ));
        let op_log = Arc::new(adapter_pg::PgPlcOperationLog::new(pool.clone()));
        let directory =
            adapter_atproto::plc_directory_from_config(&adapter_atproto::DirectoryConfig {
                endpoint: config.plc_directory_endpoint.clone(),
                enabled: config.plc_directory_submit,
            });
        let did_minter = Arc::new(adapter_atproto::RealDidMinter::new(
            key_store, op_log, directory,
        ));

        let identity_resolver = Arc::new(adapter_atproto::AtprotoIdentityResolver::new()?);
        let runtime = Runtime {
            config,
            auth: Arc::new(adapter_atproto::AtprotoAuthenticator::new(
                redirect_uri,
                pool.clone(),
                custody.signin_secrets,
                identity_resolver.clone(),
            )),
            users: Arc::new(adapter_pg::PgUserStore::new(pool.clone())),
            profile_source: Arc::new(adapter_atproto::AtprotoProfileSource::new(
                identity_resolver.clone(),
            )),
            identity_resolver,
            profile_cache: Arc::new(adapter_pg::PgProfileCache::new(
                pool.clone(),
                std::time::Duration::from_secs(60 * 60),
            )),
            profile_lookups: application::user::ProfileLookups::default(),
            did_minter,
            accounts: Arc::new(adapter_pg::PgAccountStore::new(pool.clone())),
            commissions: Arc::new(adapter_pg::PgCommissionStore::new(pool.clone())),
            changelog: Arc::new(adapter_pg::PgChangelogStore::new(pool.clone())),
            workflows: Arc::new(adapter_pg::PgWorkflowStore::new(pool.clone())),
            columns: Arc::new(adapter_pg::PgColumnStore::new(pool.clone())),
            characters: Arc::new(adapter_pg::PgCharacterStore),
            bug_reports: Arc::new(adapter_pg::PgBugReportStore::new(pool.clone())),
            files: Arc::new(adapter_pg::PgFileStore::new(pool.clone())),
            database: Arc::new(adapter_pg::PgDatabase::new(pool.clone())),
            den_mac: custody.den_mac,
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
    /// The configuration is unusable: bad `public_url`, the custody guard
    /// refused the root, or the system DNS configuration is unreadable.
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

#[cfg(test)]
mod tests;
