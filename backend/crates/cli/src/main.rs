//! `zurfur` — the terminal driving adapter's entry point.
//!
//! Parses the command line, boots tracing to **stderr** (stdout is reserved
//! for data), runs the command, and maps the outcome to the exit-code classes
//! documented on [`cli::ExitClass`]. Ctrl-C ends the run with the interrupted
//! class, never a panic.

use clap::Parser as _;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    dotenvy::dotenv().ok();
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
