//! One module per domain namespace. Each exposes a `clap::Subcommand` enum
//! (the ops) and `run(&Runtime, op) -> Result<Value, CliError>`; the root
//! [`dispatch`](crate::dispatch) routes to it. `account` and `commission`
//! land with the Engineer's operation tickets under epic ZMVP-199.

pub mod health;
pub mod migrate;
pub mod session;
