//! Commission Markup: coordinate-anchored annotation a Participant attaches to
//! a file entry in the review loop.
//!
//! Stored raw and parsed by the frontend. Coordinates are normalized 0–1 floats
//! relative to the image; containment is NOT enforced — renderers clip.
//! Validation on the way in is the only gate there will be, because the record
//! is append-only: unknown shapes and fields are refused by serde, and
//! [`Markup::validate`] enforces what serde cannot.

mod entity;
mod errors;
mod id;
mod shape;
mod value;

pub use entity::CommissionMarkup;
pub use errors::MarkupError;
pub use id::MarkupKey;
pub use shape::MarkupShape;
pub use value::Markup;
