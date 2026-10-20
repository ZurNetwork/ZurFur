//! The shared domain elements — the vocabulary every other domain crate speaks.
//!
//! [`did`] is the actor identifier, [`text`] the free-text primitives,
//! [`maturity`] the content-rating posture; [`blob`] and [`markdown`] are still
//! stubs.

pub mod blob;
pub mod did;
pub mod markdown;
pub mod maturity;
pub mod text;
