//! Zurfur's shared domain vocabulary — identifiers, text primitives, maturity
//! labels, the clock and the string-validation builder that every other domain
//! crate speaks, with no I/O of its own. It depends on no other domain crate.

pub mod datetime;
pub mod elements;
pub mod string_builder;
