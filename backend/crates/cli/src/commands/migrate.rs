//! `zurfur migrate`: migrate the private store through the custody migrator, which
//! probes the 0.1 custody rows, runs the embedded migrations and re-wraps those rows.
//! `--check` counts what a run would do and writes nothing. Exempt from the schema
//! gate: it is the fix the gate points at. Idempotent.

use adapter_pg::{CustodyCheck, CustodyMigrationError, CustodyMigrator, MigrationReport};

use crate::CliError;

/// The flags of `zurfur migrate`.
#[derive(Debug, Clone, Copy, clap::Args)]
pub struct MigrateArgs {
    /// Count the 0.1 custody rows that don't open under the configured root and the
    /// rows a run would re-wrap, writing nothing.
    #[arg(long)]
    pub check: bool,
}

/// What a run did: `{"applied":N,"version":V,"rowsRewrapped":R}`.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Migrated {
    applied: usize,
    version: Option<i64>,
    rows_rewrapped: usize,
}

impl From<MigrationReport> for Migrated {
    fn from(report: MigrationReport) -> Self {
        Migrated {
            applied: report.applied,
            version: report.version,
            rows_rewrapped: report.rows_rewrapped,
        }
    }
}

/// What a check found: `{"unopenable":N,"pending":M}`.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    unopenable: usize,
    pending: usize,
}

impl From<CustodyCheck> for Checked {
    fn from(check: CustodyCheck) -> Self {
        Checked {
            unopenable: check.unopenable,
            pending: check.pending,
        }
    }
}

/// Run `custody_migrator`, or only count under `--check`.
pub async fn run(
    custody_migrator: CustodyMigrator,
    args: MigrateArgs,
) -> Result<serde_json::Value, CliError> {
    if args.check {
        let check = custody_migrator.check().await.map_err(refusal)?;
        let body = Checked::from(check);
        Ok(serde_json::to_value(body).expect("Checked serializes"))
    } else {
        let report = custody_migrator.run().await.map_err(refusal)?;
        let body = Migrated::from(report);
        Ok(serde_json::to_value(body).expect("Migrated serializes"))
    }
}

/// A custody migration's failure as a problem: rows the configured root doesn't open
/// are a `config` problem, anything else `service_unavailable`. The text names counts
/// only; the cause goes to stderr at debug.
fn refusal(error: CustodyMigrationError) -> CliError {
    tracing::debug!(%error, "the custody migration stopped");
    match error {
        CustodyMigrationError::Unopenable { .. } => {
            CliError::infra("config", format!("migration refused: {error}"))
        }
        CustodyMigrationError::Rewrap(_)
        | CustodyMigrationError::Database(_)
        | CustodyMigrationError::Migrate(_) => {
            CliError::infra("service_unavailable", format!("migration failed: {error}"))
        }
    }
}
