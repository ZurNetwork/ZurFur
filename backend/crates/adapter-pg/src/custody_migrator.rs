//! [`CustodyMigrator`]: how a driver migrates the private store. It probes every
//! 0.1 custody row and dry-runs its re-wrap first, then runs the migrations, then
//! re-wraps the 0.1 rows into the envelope.

use std::fmt;

use domain::elements::custody::ForDidCustody;
use domain::ports::ScopedSealer;
use sqlx::PgPool;
use sqlx::migrate::MigrateError;
use zeroize::Zeroizing;

use crate::key_store::{PgKeyStore, RewrapError};
use crate::key_vault::LegacyRootKey;
use crate::{MigrationReport, latest_embedded_version, migrator, pending_migrations};

/// Migrates the private store and moves the 0.1 custody rows into the envelope.
/// Runs once: [`run`](CustodyMigrator::run) consumes it.
///
/// ```compile_fail
/// async fn migrator_is_one_shot(migrator: adapter_pg::CustodyMigrator) {
///     let _first = migrator.run().await;
///     let _second = migrator.run().await;
/// }
/// ```
///
/// ```
/// async fn migrator_runs_once(migrator: adapter_pg::CustodyMigrator) {
///     let _report = migrator.run().await;
/// }
/// ```
pub struct CustodyMigrator {
    pool: PgPool,
    legacy_root: LegacyRootKey,
    sealer: ScopedSealer<ForDidCustody>,
}

/// What [`CustodyMigrator::check`] found, without writing anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustodyCheck {
    /// The 0.1 rows that don't open under the configured root or fail the dry run.
    pub unopenable: usize,
    /// The rows a run would re-wrap.
    pub pending: usize,
}

/// Why a custody migration stopped. Names counts only: never a DID, a key id or a byte.
#[derive(Debug, thiserror::Error)]
pub enum CustodyMigrationError {
    /// The probe found rows that don't open or fail the dry run; nothing ran.
    #[error("{}", unopenable_rows(.count))]
    Unopenable { count: usize },
    /// The re-wrap stopped after the migrations; it sealed nothing.
    #[error("the custody re-wrap failed")]
    Rewrap(#[from] RewrapError),
    /// The database failed.
    #[error("the database failed during the custody migration")]
    Database(#[from] sqlx::Error),
    /// A migration failed.
    #[error("a migration failed")]
    Migrate(#[from] MigrateError),
}

impl CustodyMigrator {
    /// Build the migrator over `pool`, the 0.1 root and the DID-custody handle.
    pub fn new(
        pool: PgPool,
        legacy_root: Zeroizing<[u8; 32]>,
        sealer: ScopedSealer<ForDidCustody>,
    ) -> Self {
        Self {
            pool,
            legacy_root: LegacyRootKey::new(legacy_root),
            sealer,
        }
    }

    /// Probes and dry-runs every 0.1 row, refusing before any migration if one
    /// fails; then runs the pending migrations and re-wraps the 0.1 rows.
    pub async fn run(self) -> Result<MigrationReport, CustodyMigrationError> {
        let probe = PgKeyStore::probe_legacy(&self.pool, &self.legacy_root, &self.sealer).await?;
        if probe.unopenable > 0 {
            let refusal = CustodyMigrationError::Unopenable {
                count: probe.unopenable,
            };
            return Err(refusal);
        }
        let applied = pending_migrations(&self.pool).await?;
        migrator().run(&self.pool).await?;
        let rows_rewrapped =
            PgKeyStore::rewrap_legacy(&self.pool, &self.legacy_root, &self.sealer).await?;
        let report = MigrationReport {
            applied,
            version: latest_embedded_version(),
            rows_rewrapped,
        };
        Ok(report)
    }

    /// Counts the 0.1 rows that would refuse a run and the rows a run would
    /// re-wrap. Writes nothing.
    pub async fn check(&self) -> Result<CustodyCheck, CustodyMigrationError> {
        let probe = PgKeyStore::probe_legacy(&self.pool, &self.legacy_root, &self.sealer).await?;
        let pending = PgKeyStore::pending_rewraps(&self.pool, &probe).await?;
        let check = CustodyCheck {
            unopenable: probe.unopenable,
            pending,
        };
        Ok(check)
    }
}

impl fmt::Debug for CustodyMigrator {
    /// Prints the redacted root and the handle's purpose only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustodyMigrator")
            .field("legacy_root", &self.legacy_root)
            .field("sealer", &self.sealer)
            .finish_non_exhaustive()
    }
}

/// The refusal's wording: how many rows don't open under the configured root.
fn unopenable_rows(count: &usize) -> String {
    if *count == 1 {
        "1 custody row doesn't open under the configured root".to_owned()
    } else {
        format!("{count} custody rows don't open under the configured root")
    }
}
