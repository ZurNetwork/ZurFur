//! The domain elements — Zurfur's nouns.
//!
//! Each submodule holds one entity or closely-related cluster: its identity
//! type, its value objects, and the pure construction and invariant logic that
//! belongs with it. Every element mirrors an entry in the DESIGN glossary, which
//! is the single source of truth for what it means; [`achievement`], [`blob`],
//! [`character`] and [`markdown`] are still stubs.

pub use ::identity::elements::*;
pub use ::kernel::elements::*;
pub use ::workflow::elements::*;
