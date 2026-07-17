//! [`StringBuilder`] — the shared, explicit-rule builder every trimmed/capped
//! string newtype in [`elements`](crate::elements) validates through.
//!
//! Before this module existed, `CommissionTitle`, `ChannelPointer`, `SlotTitle`,
//! `SeatKind`/`SeatPrompt`/`SeatLink`, and `AccountName` each hand-rolled the same
//! handful of checks — trim, reject blank, cap the length, reject control
//! characters — with their own copy-pasted `if` ladder (PR #100). `StringBuilder`
//! names each check as its own method so a newtype's constructor reads as the
//! rules it enforces, in order, and a new newtype rebuilds on the same primitive
//! instead of re-deriving the ladder:
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
//!
//! There is deliberately **no negation flag** (no `allow_empty: bool`,
//! `max_len: Option<usize>`, …): each rule is its own method, applied by calling
//! it — an unwanted rule is a method you don't call, not a flag you set to
//! `false`. Rule methods take and return `Self` by value, so they chain fluently
//! without an intermediate `?` at every step.
//!
//! The builder is a **newtype over `Result`** — `StringBuilder(Result<String,
//! StringBuilderViolation>)` — because `Result` already *is* the two-state
//! machine a validation chain needs: the `Ok` arm carries the `String` every
//! rule still applies to, and the first rule to fail moves the chain to `Err`.
//! Every rule method is a `map`/`and_then` on that inner `Result`, so a rule
//! called after a failure is *structurally* a no-op — there is no
//! `if self.violation.is_none()` guard a new rule method has to remember to
//! add; the short-circuit is `and_then`'s own semantics.
//! [`build`](StringBuilder::build) is the one place the accumulated result
//! surfaces, as the plain inner `Result<String, StringBuilderViolation>`.
//! Newtypes stay the invariant carriers: `StringBuilder` only shapes the
//! string; each newtype's own `TryFrom` impl maps the one shared
//! [`StringBuilderViolation`] onto its own typed error, keeping each newtype's
//! precise 422 detail exactly as it was before this module existed.

/// Why a [`StringBuilder`] chain rejected its input — one variant per rule, each
/// carrying whatever detail a newtype's own error needs to report a precise
/// 422. Every newtype's `TryFrom` impl maps this onto its own error enum, so
/// callers never see this type directly.
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

/// The explicit-rule string-validation builder (see the module docs for the
/// full rationale and a worked example).
///
/// Every rule method takes and returns `Self` by value, so a chain like
/// `StringBuilder::new(raw).trimmed().non_empty().max_chars(512).no_control()`
/// reads as the rules being applied, in order, with no `?` until
/// [`build`](Self::build). Once a rule fails, the inner `Result` is `Err` and
/// every later rule is a structural no-op (`and_then` short-circuits) — the
/// chain always finishes, and the *first* rule to fail is the one reported.
#[derive(Debug, Clone)]
pub struct StringBuilder(Result<String, StringBuilderViolation>);

impl StringBuilder {
    /// Start a chain over `raw`. No rule has run yet — an empty, over-long, or
    /// control-character-laden `raw` is not rejected until the matching rule
    /// method is called.
    pub fn new(raw: impl Into<String>) -> Self {
        Self(Ok(raw.into()))
    }

    /// Trim leading/trailing whitespace. Not a rule that can fail — it only
    /// reshapes the value the later rules see. Skips the reallocation when the
    /// value is already trimmed.
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
    /// [`StringBuilderViolation::ControlCharacter`] — the strict gate for
    /// values that must stay a single line (a pointer, a label).
    pub fn no_control(self) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(char::is_control) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// The same rejection as [`no_control`](Self::no_control), except every
    /// character in `allowed` passes even though it is a control character —
    /// the gate for multi-line free text that should keep its line structure
    /// (e.g. `&['\n', '\r', '\t']`) while still refusing NUL, escape, and other
    /// injection-shaped characters.
    pub fn no_control_except(self, allowed: &[char]) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(|c| c.is_control() && !allowed.contains(&c)) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// Finish the chain as a plain, rule-applied `String` — for a caller with
    /// no newtype to build into (e.g. an inline API-layer check), or as the
    /// last step of a newtype's own `TryFrom` impl. Returns the first rule
    /// violation recorded, if any.
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

    // The target chain shape: each rule its own method, no `?` until the
    // finishing `build`.
    #[test]
    fn a_full_chain_trims_and_builds_into_the_newtype() {
        let probe = Probe::try_from("  hello  ".to_owned()).unwrap();
        assert_eq!(probe.0, "hello");
    }

    // Only the FIRST failing rule is reported, even though later rules would
    // also fail on the same (untrimmed, in this case irrelevant) value.
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

    // The exception list lets line-structured free text through while still
    // refusing an injection-shaped control character like NUL.
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

    // build() is the plain-String exit for a caller with no newtype — the
    // shape the notes-route inline check uses.
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

    // Once a rule fails, later rules — including trimmed() — are structural
    // no-ops: the Err arm carries no String for a rule to act on, and the
    // FIRST violation is the one build() reports (Copilot finding on PR #138).
    #[test]
    fn once_failed_later_rules_are_structural_no_ops() {
        let first_violation = StringBuilder::new("   ")
            .trimmed()
            .non_empty()
            // Everything after the failure must neither run nor re-record:
            // max_chars(0) would otherwise report TooLong, and trimmed()
            // has no String left to rewrite.
            .trimmed()
            .max_chars(0)
            .no_control()
            .build();
        assert_eq!(first_violation, Err(StringBuilderViolation::Empty));
    }
}
