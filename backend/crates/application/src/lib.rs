//! The application layer: one `#[use_case]` method per use case on a
//! per-entity namespace, called by every driver (`api`, `cli`). Authorization and orchestration live here; drivers
//! only decode, call and encode. No use case knows HTTP, sessions, argv or
//! exit codes. Depends on `domain` and `shared` only — never an adapter or
//! `composition` (`tests/dep_guard.rs`).

pub mod account;
pub mod app;
pub mod character;
pub mod commission;
pub mod common_error;
mod lazy_unit;
pub(crate) mod ports;
mod transaction;
pub mod user;

pub use app::{App, MissingPort, Ports};
pub(crate) use application_macros::use_case;
pub(crate) use lazy_unit::LazyUnit;
pub use transaction::transaction;
