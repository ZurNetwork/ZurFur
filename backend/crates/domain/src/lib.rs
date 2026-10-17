//! Zurfur's domain façade — one `domain::…` path over the domain crates, with
//! no I/O of its own.
//!
//! [`elements`] re-exports the nouns of `kernel`, `identity` and `workflow`;
//! [`ports`] re-exports their role-named traits and owns the unit of work, which
//! vends the write ports of both identity and workflow and so sits above them;
//! [`datetime`] is the one clock type and [`string_builder`] the shared
//! string-validation builder. The dependency rule runs one way: adapters point
//! at the domain, never the reverse.

pub mod elements;
pub mod ports;
pub use ::kernel::{datetime, string_builder};
