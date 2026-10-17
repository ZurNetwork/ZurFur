//! The [`User`] — Zurfur's record of a recognized visitor. A visitor's identity
//! precedes the platform, so Zurfur recognizes rather than registers: one DID
//! maps to one User forever, provisioned idempotently through
//! [`crate::ports::UserWrites`]. (DESIGN 786439)

mod entity;
mod id;

pub use entity::User;
pub use id::UserId;
