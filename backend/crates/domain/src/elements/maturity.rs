//! The platform-wide maturity rating primitive: the atproto self-label
//! vocabulary adopted as Zurfur's own — Safe / Suggestive / Nudity / Adult
//! ([`MaturityRating`]) plus an orthogonal Graphic flag, together one
//! [`Maturity`] value.
//!
//! There is no mapping layer: the network self-label a rating emits is derived
//! from it, never chosen separately.

mod errors;
mod rating;
mod value;

pub use errors::MaturityRatingError;
pub use rating::MaturityRating;
pub use value::Maturity;
