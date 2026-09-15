//! The [`Profile`] — a visitor's public, PDS-owned profile. It sits on the
//! public boundary, so the domain reads and caches it but never owns it:
//! fetched via [`crate::ports::ProfileSource`], cached behind
//! [`crate::ports::ProfileCache`].

mod display_handle;
mod entity;

pub use display_handle::DisplayHandle;
pub use entity::Profile;
