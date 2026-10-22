//! The shared domain elements — the vocabulary every other domain crate speaks.
//!
//! [`did`] is the actor identifier, [`text`] the free-text primitives,
//! [`maturity`] the content-rating posture, [`name`] the rule for a name safe to
//! show, [`level`] how visible a thing is, [`node`] a Den node; [`blob`] and
//! [`markdown`] are still stubs.

pub mod blob;
pub mod did;
pub mod level;
pub mod markdown;
pub mod maturity;
pub mod name;
pub mod node;
pub mod text;
