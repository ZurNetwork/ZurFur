//! `zurfur` — the terminal driving adapter's entry point: parses the command
//! line, boots tracing to stderr, runs the command, and maps the outcome to
//! the exit-code classes on [`cli::ExitClass`]. No `.env` loading — see this
//! crate's `NODE.md`.

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
