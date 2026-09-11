//! One module per domain namespace. Each exposes a `clap::Subcommand` enum
//! (the ops) and `run(&Runtime, op) -> Result<Value, CliError>`; the root
//! [`dispatch`](crate::dispatch) routes to it.

pub mod account;
pub mod health;
pub mod migrate;
pub mod session;
