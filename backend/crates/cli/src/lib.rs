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
//!   failure, exactly one compact JSON [`Problem`] (regardless of `--json`).
//! - exit codes are the four [`ExitClass`]es: `0` ok · `1` domain error ·
//!   `2` usage (clap's own) · `3` infrastructure (config, database, network).
//!
//! **Where commands go**: [`Command`] is the root; each domain namespace is a
//! module under [`commands`] exposing its own `clap::Subcommand` enum and a
//! `run` fn over the [`Runtime`]. `health` and `session` live here; `account`
//! and `commission` are the Engineer's operation tickets — add a module, a
//! variant on [`Command`], and an arm in [`dispatch`].

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory as _, Parser, Subcommand};
use composition::{Config, ConnectError, Runtime};

pub mod commands;
mod output;
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

/// The root command tree. One variant per domain namespace (plus the
/// tool-level `completions`).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Probe the database the way `GET /health` does.
    Health,
    /// The acting identity: `login`, `logout`, `whoami`.
    Session {
        #[command(subcommand)]
        op: commands::session::SessionOp,
    },
    /// Print a shell completion script to stdout.
    Completions {
        /// The shell to generate for.
        shell: clap_complete::Shell,
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
/// `completions` — never touch a database.
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
        command => {
            let runtime = connect(cli.config_dir).await?;
            let value = dispatch(&runtime, command).await?;
            Ok(Output::json(value, format))
        }
    }
}

/// Route a backend-needing [`Command`] to its namespace module over the booted
/// [`Runtime`]. The in-process harness calls this directly with a `Runtime`
/// over the in-memory fakes — no database, no process spawn.
pub async fn dispatch(runtime: &Runtime, command: Command) -> Result<serde_json::Value, CliError> {
    match command {
        Command::Health => commands::health::run(runtime).await,
        Command::Session { op } => commands::session::run(runtime, op).await,
        Command::Completions { .. } => {
            unreachable!("completions never reach dispatch; `run` answers it without a runtime")
        }
    }
}

/// Load the [`Config`] (honoring `--config-dir`) and wire the live adapters.
/// Every failure here is infrastructure: exit class 3.
async fn connect(config_dir: Option<PathBuf>) -> Result<Runtime, CliError> {
    let config = Config::load_from(config_dir).map_err(|e| CliError::infra("config", e))?;
    Runtime::connect(config).await.map_err(|e| match e {
        ConnectError::Database(_) => CliError::infra("database_unreachable", e),
        ConnectError::Setup(_) => CliError::infra("runtime", e),
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
