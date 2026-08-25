//! `zurfur` — the terminal driving adapter's entry point.
//!
//! Parses the command line, boots tracing to **stderr** (stdout is reserved
//! for data), runs the command, and maps the outcome to the exit-code classes
//! documented on [`cli::ExitClass`]. Ctrl-C ends the run with the interrupted
//! class, never a panic.
//!
//! No `.env` loading, deliberately: `dotenvy::dotenv()` walks every parent of
//! the CWD, so a user binary run from an arbitrary directory would take
//! `DATABASE_URL` / the root key / `ZURFUR_CLI_HOME` from whatever `.env` sits
//! above it (security review, ZMVP-203 F1). The dev loop gets `.env` through
//! `just` (`dotenv-load`); anything else sets the environment explicitly.

use clap::Parser as _;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args = cli::Cli::parse();
    cli::init_tracing();

    let run = cli::run(args);
    let interrupt = tokio::signal::ctrl_c();
    let outcome = tokio::select! {
        outcome = run => outcome,
        _ = interrupt => Err(cli::CliError::interrupted()),
    };
    cli::finish(outcome)
}
