//! [`StringBuilder`] — the shared, explicit-rule builder every trimmed/capped
//! string newtype in [`elements`](crate::elements) validates through.
//!
//! Each rule is its own method, so a newtype's constructor reads as the rules it
//! enforces, in order; there are no negation flags. A rule called after a
//! failure is a structural no-op, so [`build`](StringBuilder::build) always
//! reports the FIRST violation. Newtypes stay the invariant carriers: each maps
//! the shared [`StringBuilderViolation`] onto its own typed error.
//!
//! ```
//! # use domain::string_builder::{StringBuilder, StringBuilderViolation};
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

/// Why a [`StringBuilder`] chain rejected its input — one variant per rule.
/// Newtypes map this onto their own error enum, so callers never see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringBuilderViolation {
    /// [`StringBuilder::non_empty`] found nothing left after trimming.
    Empty,
    /// [`StringBuilder::max_chars`] found more than `max` trimmed `char`s;
    /// `len` is the offending count.
    TooLong {
        /// The configured cap.
        max: usize,
        /// The offending length, in `char`s.
        len: usize,
    },
    /// [`StringBuilder::no_control`] or [`StringBuilder::no_control_except`]
    /// found a control character the rule doesn't allow.
    ControlCharacter,
}

impl std::fmt::Display for StringBuilderViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringBuilderViolation::Empty => write!(f, "must not be empty"),
            StringBuilderViolation::TooLong { max, len } => {
                write!(f, "is {len} characters; the max is {max}")
            }
            StringBuilderViolation::ControlCharacter => {
                write!(f, "must not contain control characters")
            }
        }
    }
}

impl std::error::Error for StringBuilderViolation {}

/// The explicit-rule string-validation builder. Rule methods take and return
/// `Self`, so a chain reads as the rules applied in order with no `?` until
/// [`build`](Self::build); once one fails, the rest short-circuit.
#[derive(Debug, Clone)]
pub struct StringBuilder(Result<String, StringBuilderViolation>);

impl StringBuilder {
    /// Start a chain over `raw`. No rule has run yet.
    pub fn new(raw: impl Into<String>) -> Self {
        Self(Ok(raw.into()))
    }

    /// Trim leading/trailing whitespace. Cannot fail.
    pub fn trimmed(self) -> Self {
        Self(self.0.map(|s| {
            let trimmed = s.trim();
            if s.len() == trimmed.len() {
                s
            } else {
                trimmed.to_owned()
            }
        }))
    }

    /// Reject an empty value with [`StringBuilderViolation::Empty`]. Call after
    /// [`trimmed`](Self::trimmed) to reject whitespace-only input too.
    pub fn non_empty(self) -> Self {
        Self(self.0.and_then(|s| {
            if s.is_empty() {
                Err(StringBuilderViolation::Empty)
            } else {
                Ok(s)
            }
        }))
    }

    /// Reject a value longer than `max` trimmed `char`s with
    /// [`StringBuilderViolation::TooLong`].
    pub fn max_chars(self, max: usize) -> Self {
        Self(self.0.and_then(|s| {
            let len = s.chars().count();

            if len > max {
                Err(StringBuilderViolation::TooLong { max, len })
            } else {
                Ok(s)
            }
        }))
    }

    /// Reject any [`char::is_control`] character with
    /// [`StringBuilderViolation::ControlCharacter`].
    pub fn no_control(self) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(char::is_control) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// As [`no_control`](Self::no_control), except every character in
    /// `allowed` passes — the gate for multi-line free text.
    pub fn no_control_except(self, allowed: &[char]) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(|c| c.is_control() && !allowed.contains(&c)) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// Finish the chain as a plain, rule-applied `String`, or the first rule
    /// violation recorded.
    pub fn build(self) -> Result<String, StringBuilderViolation> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Probe(String);

    #[derive(Debug, PartialEq, Eq)]
    enum ProbeError {
        Empty,
        TooLong { max: usize, len: usize },
        ControlCharacter,
    }

    impl TryFrom<String> for Probe {
        type Error = ProbeError;

        fn try_from(raw: String) -> Result<Self, Self::Error> {
            StringBuilder::new(raw)
                .trimmed()
                .non_empty()
                .max_chars(512)
                .no_control()
                .build()
                .map(Self)
                .map_err(|violation| match violation {
                    StringBuilderViolation::Empty => ProbeError::Empty,
                    StringBuilderViolation::TooLong { max, len } => {
                        ProbeError::TooLong { max, len }
                    }
                    StringBuilderViolation::ControlCharacter => ProbeError::ControlCharacter,
                })
        }
    }

    // Each rule its own method, no `?` until the finishing `build`.
    #[test]
    fn a_full_chain_trims_and_builds_into_the_newtype() {
        let probe = Probe::try_from("  hello  ".to_owned()).unwrap();
        assert_eq!(probe.0, "hello");
    }

    // Only the FIRST failing rule is reported.
    #[test]
    fn only_the_first_violation_is_reported() {
        let result = StringBuilder::new("   ")
            .trimmed()
            .non_empty()
            .max_chars(1)
            .no_control()
            .build();
        assert_eq!(result, Err(StringBuilderViolation::Empty));
    }

    #[test]
    fn max_chars_reports_the_cap_and_offending_length() {
        let result = StringBuilder::new("hello")
            .trimmed()
            .non_empty()
            .max_chars(3)
            .build();
        assert_eq!(
            result,
            Err(StringBuilderViolation::TooLong { max: 3, len: 5 })
        );
    }

    #[test]
    fn no_control_rejects_any_control_character() {
        let result = StringBuilder::new("a\nb")
            .trimmed()
            .non_empty()
            .no_control()
            .build();
        assert_eq!(result, Err(StringBuilderViolation::ControlCharacter));
    }

    // The exception list lets line-structured free text through, but not NUL.
    #[test]
    fn no_control_except_allows_only_the_listed_characters() {
        let allowed = StringBuilder::new("a\nb\tc")
            .trimmed()
            .non_empty()
            .no_control_except(&['\n', '\t'])
            .build()
            .unwrap();
        assert_eq!(allowed, "a\nb\tc");

        let rejected = StringBuilder::new("a\0b")
            .trimmed()
            .non_empty()
            .no_control_except(&['\n', '\t'])
            .build();
        assert_eq!(rejected, Err(StringBuilderViolation::ControlCharacter));
    }

    // build() is the plain-String exit for a caller with no newtype.
    #[test]
    fn build_returns_the_plain_rule_applied_string() {
        assert_eq!(
            StringBuilder::new("  hi  ").trimmed().non_empty().build(),
            Ok("hi".to_owned())
        );
        assert_eq!(
            StringBuilder::new("   ").trimmed().non_empty().build(),
            Err(StringBuilderViolation::Empty)
        );
    }

    // Once a rule fails, later rules — trimmed() included — are structural
    // no-ops, and build() reports the first violation.
    #[test]
    fn once_failed_later_rules_are_structural_no_ops() {
        let first_violation = StringBuilder::new("   ")
            .trimmed()
            .non_empty()
            // max_chars(0) would otherwise report TooLong.
            .trimmed()
            .max_chars(0)
            .no_control()
            .build();
        assert_eq!(first_violation, Err(StringBuilderViolation::Empty));
    }
}
