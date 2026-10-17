use super::HANDLE_MAX_LEN;
use super::LABEL_MAX_LEN;

/// Why a string was rejected as a [`Handle`] or an [`AtHandle`](super::AtHandle)
/// — one variant per failure class, each rendering a human message via
/// [`Display`](std::fmt::Display). Both parsers share the syntax variants.
/// Only a `Handle` refuses a punycode label, at its parse; a reserved label and
/// the namespace's apex are refused at the claim, by
/// [`Handle::ensure_unreserved`] against the configured namespace.
///
/// ```
/// use identity::elements::handle::{Handle, HandleError};
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
    /// A label begins with `xn--`. Only a [`Handle`] refuses it, in both
    /// namespaces; an `AtHandle` accepts it.
    #[error("punycode (`xn--`) handle labels are not allowed")]
    PunycodeLabel,
    /// A member of the configured namespace whose leftmost label is reserved;
    /// carries that label. Only a [`Handle`] refuses it, through
    /// [`Handle::ensure_unreserved`].
    #[error("`{0}` is a reserved label in Zurfur's handle namespace")]
    ReservedLabel(String),
    /// The configured namespace's own apex, which no one claims. Only a
    /// [`Handle`] refuses it, through [`Handle::ensure_unreserved`].
    #[error("the apex of Zurfur's handle namespace cannot be claimed as a handle")]
    NamespaceApex,
}

/// Why a string was rejected as a [`HandleDomain`].
///
/// ```
/// use identity::elements::handle::{HandleDomain, HandleDomainError, HandleError};
///
/// assert_eq!("  . ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
/// assert_eq!(
///     "*.zurfur.app".parse::<HandleDomain>(),
///     Err(HandleDomainError::NotAHostname(HandleError::InvalidChar('*')))
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandleDomainError {
    /// Empty once normalized.
    #[error("the handle domain must not be empty")]
    Empty,
    /// Not a hostname a handle can live under: it breaks a [`Handle`] syntax
    /// rule; carries the rule.
    #[error("the handle domain must be a bare hostname: {0}")]
    NotAHostname(HandleError),
}

#[cfg(test)]
mod tests;
