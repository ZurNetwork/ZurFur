//! `zurfur`: the terminal driving adapter. Calls the same application-layer
//! use cases as `api`, in-process via [`composition::Runtime`] — no HTTP, no
//! bearer token. Commands are one-shot `clap` subcommands under [`commands`].
//! See this crate's `NODE.md` for the stdout/stderr/exit-code conventions
//! and the problem-code vocabulary.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory as _, Parser, Subcommand};
use composition::{Config, ConnectError, Runtime};

pub mod commands;
mod confirm;
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

/// The root command tree: [`Command`]s that never touch the backend, plus
/// the [`BackendCommand`]s that do — the split keeps [`dispatch`] exhaustive.
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

/// The commands that run over a booted [`Runtime`], one variant per domain
/// namespace. `session logout` is the one member that `run` answers without
/// booting anything.
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
    /// Accounts: `create`, `delete`.
    Account {
        #[command(subcommand)]
        op: commands::account::AccountOp,
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

/// Run the parsed command line end to end: commands needing the backend boot
/// the shared [`Runtime`] first; `completions` and `session logout` never
/// touch config or a database. Returns the [`Output`] for stdout, or the
/// [`CliError`] that becomes the stderr [`Problem`].
pub async fn run(cli: Cli) -> Result<Output, CliError> {
    let format = output::Format::from_flag(cli.json);
    match cli.command {
        Command::Completions { shell } => {
            let mut buffer = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "zurfur", &mut buffer);
            Ok(Output::raw(buffer))
        }
        // Logout must work even when config/database are broken — checked first.
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
/// [`Runtime`], with the identity file at `identity_path`. Tests call this
/// directly over in-memory fakes — no database, no process spawn.
pub async fn dispatch(
    runtime: &Runtime,
    identity_path: &Path,
    command: BackendCommand,
) -> Result<serde_json::Value, CliError> {
    match command {
        BackendCommand::Health => commands::health::run(runtime).await,
        BackendCommand::Migrate => commands::migrate::run(runtime).await,
        BackendCommand::Session { op } => commands::session::run(runtime, identity_path, op).await,
        BackendCommand::Account { op } => commands::account::run(runtime, identity_path, op).await,
    }
}

/// The schema-drift gate: refuses a database whose applied migrations are
/// behind the embedded set, ahead of it, or absent, so the CLI never runs
/// against a schema it wasn't built for. `migrate` and `health` are exempt
/// (the fix, and the reporter).
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
        // service_unavailable = down dependency; internal_error = broken boot.
        ConnectError::Database(_) => CliError::infra("service_unavailable", e),
        ConnectError::Setup(_) => CliError::infra("internal_error", e),
    })
}

/// Render a config-load failure without echoing any value: figment prints
/// the parsed value on a type mismatch, which for an env var can be the
/// secret. Only shape-safe kinds pass through; everything else is a generic
/// detail with the parser's message behind `RUST_LOG=debug`.
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
