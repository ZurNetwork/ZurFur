//! Text primitives with one rule each, shared by the elements that carry free
//! text: a [`NonEmptyString`] is trimmed and never blank.

mod errors;
mod value;

pub use errors::NonEmptyStringError;
pub use value::NonEmptyString;
