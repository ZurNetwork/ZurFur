//! Text primitives shared by the elements that carry free text: a
//! [`NonEmptyString`] is trimmed and never blank, and [`StoredText`] is text as
//! the store holds it, loaded without re-checking.

mod errors;
mod rows;
mod value;

pub use errors::NonEmptyStringError;
pub use rows::StoredText;
pub use value::NonEmptyString;
