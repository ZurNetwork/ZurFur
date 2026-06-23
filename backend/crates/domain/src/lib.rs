//! Zurfur's pure domain core — the entities, value objects, and ports of the
//! art-commission platform, with no I/O of its own.
//!
//! This crate is the centre of the ports-and-adapters architecture (see
//! DESIGN/"Domains and Applications"): it depends on nothing app-specific and is
//! depended on by every adapter (`adapter-pg`, `adapter-atproto`, `adapter-mem`)
//! and composed by `api`. The dependency rule runs one way only — adapters point
//! at the domain, never the reverse.
//!
//! - [`elements`] — the domain elements: entities like [`elements::account::Account`]
//!   and [`elements::user::User`], their ids and value objects, plus stubs for
//!   namespaces (Character, Golem, Commission, …) not yet built out.
//! - [`ports`] — traits named by the role they play *for* the domain (a
//!   `UserRepo`, an `Authenticator`), implemented by the adapter crates. These
//!   are the seams the domain reaches the outside world through.
//! - [`datetime`] — the single clock type the domain speaks in.
//!
//! The `domain` crate is transitional: as namespaces are built it splits into
//! per-domain crates (`identity`, `gallery`, `workflow`, `plugin`).

pub mod datetime;
pub mod elements;
pub mod ports;
