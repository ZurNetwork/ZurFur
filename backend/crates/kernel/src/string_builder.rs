//! [`StringBuilder`] — the shared, explicit-rule builder every trimmed/capped
//! string newtype in `elements` validates through.
//!
//! Each rule is its own method, so a newtype's constructor reads as the rules it
//! enforces, in order; there are no negation flags. A rule called after a
//! failure is a structural no-op, so [`build`](StringBuilder::build) always
//! reports the FIRST violation. Newtypes stay the invariant carriers: each maps
//! the shared [`StringBuilderViolation`] onto its own typed error.
//!
//! The Unicode tables the rules and a printer consult are public too:
//! [`is_bidi_control`], and [`is_unsafe_to_print`] with its parts [`is_format`] and
//! [`is_default_ignorable`].
//!
//! ```
//! # use kernel::string_builder::{StringBuilder, StringBuilderViolation};
//! # #[derive(Debug, PartialEq, Eq)]
//! # struct Example(String);
//! # #[derive(Debug, PartialEq, Eq)]
//! # enum ExampleError { Empty, TooLong, ControlCharacter }
//! # impl TryFrom<String> for Example {
//! #     type Error = ExampleError;
//! #     fn try_from(raw: String) -> Result<Self, Self::Error> {
//! #         StringBuilder::new(raw)
//! #             .trimmed()
//! #             .non_empty()
//! #             .max_chars(512)
//! #             .no_control()
//! #             .build()
//! #             .map(Self)
//! #             .map_err(|violation| match violation {
//! #                 StringBuilderViolation::Empty => ExampleError::Empty,
//! #                 StringBuilderViolation::TooLong { .. } => ExampleError::TooLong,
//! #                 StringBuilderViolation::ControlCharacter => ExampleError::ControlCharacter,
//! #             })
//! #     }
//! # }
//! let example = Example::try_from("  hello  ".to_owned()).unwrap();
//! assert_eq!(example.0, "hello");
//! ```

mod builder;
mod errors;
mod unicode;

pub use builder::StringBuilder;
pub use errors::StringBuilderViolation;
pub use unicode::{
    DEFAULT_IGNORABLE, FORMAT_CHARACTERS, is_bidi_control, is_default_ignorable, is_format,
    is_unsafe_to_print,
};
