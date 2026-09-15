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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleError {
    /// Empty once normalized.
    Empty,
    /// Longer than [`HANDLE_MAX_LEN`] chars overall; carries the length.
    TooLong(usize),
    /// Fewer than two dot-separated segments (e.g. bare `"alice"`).
    TooFewSegments,
    /// A dot-separated segment is empty (e.g. `"alice..app"`).
    EmptySegment,
    /// A segment is longer than [`LABEL_MAX_LEN`] chars; carries the length.
    SegmentTooLong(usize),
    /// A character outside the `[a-z0-9-]` charset; carries the char.
    InvalidChar(char),
    /// A segment starts or ends with a hyphen.
    HyphenEdge,
    /// The rightmost (top-level) segment starts with a digit.
    TldLeadingDigit,
    /// The rightmost segment is a reserved TLD (e.g. `.local`); carries it.
    ReservedTld(String),
    /// Some label begins with `xn--`. Rejected in both namespaces.
    PunycodeLabel,
    /// The leftmost label of a `*.zurfur.app` handle is reserved; carries it.
    ReservedLabel(String),
}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleError::Empty => write!(f, "handle must not be empty"),
            HandleError::TooLong(len) => {
                write!(f, "handle is {len} chars; the max is {HANDLE_MAX_LEN}")
            }
            HandleError::TooFewSegments => write!(
                f,
                "handle must have at least two segments (e.g. `name.zurfur.app`)"
            ),
            HandleError::EmptySegment => write!(f, "handle has an empty segment"),
            HandleError::SegmentTooLong(len) => {
                write!(
                    f,
                    "handle segment is {len} chars; the max is {LABEL_MAX_LEN}"
                )
            }
            HandleError::InvalidChar(c) => write!(
                f,
                "handle contains an invalid character {c:?}; only a-z, 0-9, and '-' are allowed"
            ),
            HandleError::HyphenEdge => {
                write!(f, "a handle segment must not start or end with a hyphen")
            }
            HandleError::TldLeadingDigit => {
                write!(
                    f,
                    "the rightmost handle segment must not start with a digit"
                )
            }
            HandleError::ReservedTld(tld) => {
                write!(
                    f,
                    "`.{tld}` is a reserved top-level domain and cannot be a handle"
                )
            }
            HandleError::PunycodeLabel => {
                write!(f, "punycode (`xn--`) handle labels are not allowed")
            }
            HandleError::ReservedLabel(label) => write!(
                f,
                "`{label}` is a reserved label in the .zurfur.app namespace"
            ),
        }
    }
}

impl std::error::Error for HandleError {}

/// Why a string was rejected as a [`HandleDomain`].
///
/// ```
/// use domain::elements::handle::{HandleDomain, HandleDomainError};
///
/// assert_eq!("  . ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleDomainError {
    /// Empty once normalized.
    Empty,
}

impl std::fmt::Display for HandleDomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleDomainError::Empty => write!(f, "the handle domain must not be empty"),
        }
    }
}

impl std::error::Error for HandleDomainError {}

#[cfg(test)]
mod tests;
