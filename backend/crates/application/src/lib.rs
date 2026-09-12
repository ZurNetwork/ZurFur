//! The application layer: one plain `async fn` per use case, called by every
//! driver (`api`, `cli`). Authorization and orchestration live here; drivers
//! only decode, call and encode. No use case knows HTTP, sessions, argv or
//! exit codes. Depends on `domain` and `shared` only — never an adapter or
//! `composition` (`tests/dep_guard.rs`).

pub mod account;
pub mod app;
pub mod commission;
pub(crate) mod ports;
mod transaction;
pub mod user;

pub use app::{App, MissingPort, Ports};
pub use transaction::transaction;
