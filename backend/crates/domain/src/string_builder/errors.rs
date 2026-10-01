/// Why a [`StringBuilder`](super::StringBuilder) chain rejected its input —
/// one variant per rule. Newtypes map this onto their own error enum, so
/// callers never see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StringBuilderViolation {
    /// [`StringBuilder::non_empty`](super::StringBuilder::non_empty) found
    /// nothing left after trimming.
    #[error("must not be empty")]
    Empty,
    /// [`StringBuilder::max_chars`](super::StringBuilder::max_chars) found
    /// more than `max` trimmed `char`s; `len` is the offending count.
    #[error("is {len} characters; the max is {max}")]
    TooLong {
        /// The configured cap.
        max: usize,
        /// The offending length, in `char`s.
        len: usize,
    },
    /// [`StringBuilder::no_control`](super::StringBuilder::no_control) or
    /// [`StringBuilder::no_control_except`](super::StringBuilder::no_control_except)
    /// found a control character the rule doesn't allow.
    #[error("must not contain control characters")]
    ControlCharacter,
}
