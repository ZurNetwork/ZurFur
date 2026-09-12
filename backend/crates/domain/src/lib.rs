//! Zurfur's pure domain core — the entities, value objects and ports of the
//! art-commission platform, with no I/O of its own. (DESIGN 11763713)
//!
//! [`elements`] holds the nouns, [`ports`] the role-named traits the adapters
//! implement, [`datetime`] the one clock type, and [`string_builder`] the shared
//! string-validation builder. The dependency rule runs one way: adapters point
//! at the domain, never the reverse.

pub mod datetime;
pub mod elements;
pub mod ports;
pub mod string_builder;
