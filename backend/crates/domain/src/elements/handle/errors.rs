use super::HANDLE_MAX_LEN;
use super::LABEL_MAX_LEN;

/// Why a string was rejected as a [`Handle`] — one variant per failure class,
/// each rendering a human message via [`Display`](std::fmt::Display).
///
/// ```
/// use domain::elements::handle::{Handle, HandleError};
///
/// assert_eq!("".parse::<Handle>(), Err(HandleError::Empty));
/// assert_eq!("alice".parse::<Handle>(), Err(HandleError::TooFewSegments));
/// assert_eq!("foo.local".parse::<Handle>(), Err(HandleError::ReservedTld("local".into())));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleError {
    /// Empty once normalized.
    #[error("handle must not be empty")]
    Empty,
    /// Longer than [`HANDLE_MAX_LEN`] chars overall; carries the length.
    #[error("handle is {0} chars; the max is {HANDLE_MAX_LEN}")]
    TooLong(usize),
    /// Fewer than two dot-separated segments (e.g. bare `"alice"`).
    #[error("handle must have at least two segments (e.g. `name.zurfur.app`)")]
    TooFewSegments,
    /// A dot-separated segment is empty (e.g. `"alice..app"`).
    #[error("handle has an empty segment")]
    EmptySegment,
    /// A segment is longer than [`LABEL_MAX_LEN`] chars; carries the length.
    #[error("handle segment is {0} chars; the max is {LABEL_MAX_LEN}")]
    SegmentTooLong(usize),
    /// A character outside the `[a-z0-9-]` charset; carries the char.
    #[error("handle contains an invalid character {0:?}; only a-z, 0-9, and '-' are allowed")]
    InvalidChar(char),
    /// A segment starts or ends with a hyphen.
    #[error("a handle segment must not start or end with a hyphen")]
    HyphenEdge,
    /// The rightmost (top-level) segment starts with a digit.
    #[error("the rightmost handle segment must not start with a digit")]
    TldLeadingDigit,
    /// The rightmost segment is a reserved TLD (e.g. `.local`); carries it.
    #[error("`.{0}` is a reserved top-level domain and cannot be a handle")]
    ReservedTld(String),
    /// Some label begins with `xn--`. Rejected in both namespaces.
    #[error("punycode (`xn--`) handle labels are not allowed")]
    PunycodeLabel,
    /// The leftmost label of a `*.zurfur.app` handle is reserved; carries it.
    #[error("`{0}` is a reserved label in the .zurfur.app namespace")]
    ReservedLabel(String),
}

/// Why a string was rejected as a [`HandleDomain`].
///
/// ```
/// use domain::elements::handle::{HandleDomain, HandleDomainError};
///
/// assert_eq!("  . ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleDomainError {
    /// Empty once normalized.
    #[error("the handle domain must not be empty")]
    Empty,
}

#[cfg(test)]
mod tests;
