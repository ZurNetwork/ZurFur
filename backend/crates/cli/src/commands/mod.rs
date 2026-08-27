//! One module per domain namespace. Each exposes a `clap::Subcommand` enum
//! (the ops) and `run(&Runtime, op) -> Result<Value, CliError>`; the root
//! [`dispatch`](crate::dispatch) routes to it. `account` opened with
//! `create` (ZMVP-205); the rest of `account` and all of `commission` land
//! with the operation tickets under epic ZMVP-199.

pub mod account;
pub mod health;
pub mod migrate;
pub mod session;
