//! `zurfur` — the terminal driving adapter (epic ZMVP-199).
//!
//! A second composition consumer beside `api`: it links `domain` and the live
//! adapters **in-process** through the shared [`composition::Runtime`] — no
//! HTTP, no contract, no bearer token — so every use case the CLI reaches is
//! by construction reachable from any driver. Commands are one-shot `clap`
//! subcommands (a REPL is later sugar over the same parser; Engineer ruling
//! 2026-08-24).
//!
//! **Conventions** (ZMVP-201) — every command honors these, tested by the
//! process-level harness in `tests/`:
//! - stdout carries exactly one JSON value + newline on success (pretty by
//!   default, compact under `--json`); nothing else ever goes to stdout.
//! - stderr carries diagnostics — `tracing` under `RUST_LOG` — and, on
//!   failure, one compact JSON [`Problem`] as its **last line** (regardless of
//!   `--json`); scripts parse `stderr.lines().last()`.
//! - exit codes are the four [`ExitClass`]es: `0` ok · `1` domain error ·
//!   `2` usage (clap's own) · `3` infrastructure (config, database, network).
//! - problem `code`s reuse the API's vocabulary (`api/src/problem.rs`, DD
//!   23592962) wherever the same refusal exists there — `not_authenticated`,
//!   `service_unavailable`, `internal_error` — and add CLI-only codes
//!   (`config`, `identity_*`, `not_implemented`) where the API has none.
//!
//! **Where commands go**: [`Command`] is the root; each domain namespace is a
//! module under [`commands`] exposing its own `clap::Subcommand` enum and a
//! `run` fn over the [`Runtime`]. `health` and `session` live here; `account`
//! and `commission` are the Engineer's operation tickets — add a module, a
//! variant on [`Command`], and an arm in [`dispatch`]. A command that acts as
//! someone resolves its [`principal::Principal`] first — the one shared path
//! from the identity file to a `User` (ZMVP-203).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory as _, Parser, Subcommand};
use composition::{Config, ConnectError, Runtime};

pub mod commands;
pub mod identity;
mod output;
pub mod principal;
mod problem;

pub use output::Output;
pub use problem::{CliError, ExitClass, Problem};

/// The parsed command line: global flags plus one [`Command`].
#[derive(Debug, Parser)]
#[command(name = "zurfur", version, about = "Zurfur from the terminal", long_about = None)]
pub struct Cli {
    /// Print compact JSON on stdout (default: pretty-printed).
    #[arg(long, global = true)]
    pub json: bool,
    /// Directory holding the `<ZURFUR_ENV>.toml` profile (default: the repo's
    /// `backend/config`, or `ZURFUR_CONFIG_DIR`).
    #[arg(long, global = true, value_name = "DIR")]
    pub config_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

/// The root command tree: the tool-level commands that never touch the
/// backend, and the [`BackendCommand`]s that do — split so [`dispatch`] can
/// only ever be handed the latter (no unreachable arm, by construction).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print a shell completion script to stdout.
    Completions {
        /// The shell to generate for.
        shell: clap_complete::Shell,
    },
    #[command(flatten)]
    Backend(BackendCommand),
}

/// The commands that run over a booted [`Runtime`] — one variant per domain
/// namespace. `session logout` is the one member `run` answers *without*
/// booting anything (it must work when the stack is broken).
#[derive(Debug, Subcommand)]
pub enum BackendCommand {
    /// Probe the database the way `GET /health` does (reports the schema
    /// state; never refuses on it).
    Health,
    /// Apply pending migrations — the ONLY way the CLI changes the schema.
    Migrate,
    /// The acting identity: `login`, `logout`, `whoami`.
    Session {
        #[command(subcommand)]
        op: commands::session::SessionOp,
    },
}

/// Boot `tracing` to **stderr** under `RUST_LOG` (default `warn`), so stdout
/// stays the data channel. Idempotent per process; a second call is ignored.
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

/// Run the parsed command line end to end: commands that need the backend
/// boot the shared [`Runtime`] (config → live adapters) first; the rest —
/// `completions`, `session logout` — never touch config or a database.
///
/// Returns the JSON value for stdout, or the [`CliError`] that becomes the
/// stderr [`Problem`] + exit class.
pub async fn run(cli: Cli) -> Result<Output, CliError> {
    let format = output::Format::from_flag(cli.json);
    match cli.command {
        Command::Completions { shell } => {
            let mut buffer = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "zurfur", &mut buffer);
            Ok(Output::raw(buffer))
        }
        // Forgetting the identity must work when everything else is broken —
        // no config, no database (security review, ZMVP-203 F3).
        Command::Backend(BackendCommand::Session {
            op: commands::session::SessionOp::Logout,
        }) => {
            let identity_path = identity::default_path()?;
            let value = commands::session::logout(&identity_path)?;
            Ok(Output::json(value, format))
        }
        Command::Backend(command) => {
            let identity_path = identity::default_path()?;
            let runtime = connect(cli.config_dir).await?;
            require_current_schema(&runtime, &command).await?;
            let value = dispatch(&runtime, &identity_path, command).await?;
            Ok(Output::json(value, format))
        }
    }
}

/// Route a [`BackendCommand`] to its namespace module over the booted
/// [`Runtime`], with the identity file at `identity_path` (see
/// [`identity::default_path`]). The in-process harness calls this directly
/// with a `Runtime` over the in-memory fakes and a temp dir — no database, no
/// process spawn.
pub async fn dispatch(
    runtime: &Runtime,
    identity_path: &Path,
    command: BackendCommand,
) -> Result<serde_json::Value, CliError> {
    match command {
        BackendCommand::Health => commands::health::run(runtime).await,
        BackendCommand::Migrate => commands::migrate::run(runtime).await,
        BackendCommand::Session { op } => commands::session::run(runtime, identity_path, op).await,
    }
}

/// The schema-drift gate (ZMVP-206, Engineer ruling: option B): a command
/// that acts on data refuses a database whose applied migrations are behind
/// the embedded set, ahead of it, or has no ledger at all — so the CLI never runs against
/// a schema it wasn't built for and never migrates by accident. `migrate` is
/// exempt (it is the fix); `health` is exempt (it reports the state instead).
pub async fn require_current_schema(
    runtime: &Runtime,
    command: &BackendCommand,
) -> Result<(), CliError> {
    if matches!(command, BackendCommand::Migrate | BackendCommand::Health) {
        return Ok(());
    }
    let status = adapter_pg::schema_status(&runtime.pool)
        .await
        .map_err(|e| CliError::infra("service_unavailable", format!("schema check failed: {e}")))?;
    match status {
        adapter_pg::SchemaStatus::Current => Ok(()),
        adapter_pg::SchemaStatus::Behind { pending } => Err(CliError::infra(
            "service_unavailable",
            format!("schema is {pending} migration(s) behind — run `zurfur migrate`"),
        )),
        adapter_pg::SchemaStatus::Unknown => Err(CliError::infra(
            "service_unavailable",
            "database has no schema yet — run `zurfur migrate`",
        )),
        adapter_pg::SchemaStatus::Ahead { unknown } => Err(CliError::infra(
            "service_unavailable",
            format!(
                "schema has {unknown} migration(s) this build does not know — upgrade `zurfur`"
            ),
        )),
    }
}

/// Load the [`Config`] (honoring `--config-dir`) and wire the live adapters.
/// Every failure here is infrastructure: exit class 3.
async fn connect(config_dir: Option<PathBuf>) -> Result<Runtime, CliError> {
    let config = Config::load_from(config_dir).map_err(|e| config_problem(&e))?;
    Runtime::connect(config).await.map_err(|e| match e {
        // The API's codes (DD 23592962), one vocabulary across both drivers:
        // a down dependency is `service_unavailable`, a broken boot is
        // `internal_error`. The `detail` keeps the two database cases apart.
        ConnectError::Database(_) => CliError::infra("service_unavailable", e),
        ConnectError::Setup(_) => CliError::infra("internal_error", e),
    })
}

/// Render a config-load failure without echoing any value. figment prints
/// the parsed value on a type mismatch — which for an env var IS the secret
/// (security review, ZMVP-203 F4) — so only the shape-safe kinds pass
/// through; everything else is a generic detail with the parser's message
/// behind `RUST_LOG=debug`.
fn config_problem(error: &figment::Error) -> CliError {
    use figment::error::Kind;
    let detail = match &error.kind {
        Kind::MissingField(name) => format!("missing configuration key `{name}`"),
        Kind::UnknownVariant(found, expected) if !looks_secret(&error.path) => {
            format!(
                "unknown value `{found}` for `{}`; expected one of {expected:?}",
                error.path.join(".")
            )
        }
        _ => {
            tracing::debug!(%error, "configuration failed to load");
            format!(
                "configuration key `{}` could not be loaded (RUST_LOG=debug shows the parser's message)",
                error.path.join(".")
            )
        }
    };
    CliError::infra("config", detail)
}

/// Keys whose values must never reach stderr, even in an "unknown variant"
/// message.
fn looks_secret(path: &[String]) -> bool {
    path.iter().any(|segment| {
        let key = segment.to_ascii_lowercase();
        key.contains("key")
            || key.contains("secret")
            || key.contains("url")
            || key.contains("password")
    })
}

/// Print the outcome per the conventions and yield the process exit code:
/// success → the [`Output`] on stdout, exit `0`; failure → one compact
/// [`Problem`] on stderr, the error's [`ExitClass`].
pub fn finish(outcome: Result<Output, CliError>) -> ExitCode {
    match outcome {
        Ok(output) => {
            output.write_stdout();
            ExitCode::SUCCESS
        }
        Err(error) => {
            error.problem().write_stderr();
            error.class().exit_code()
        }
    }
}
