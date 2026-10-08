use std::str::FromStr;

use super::HandleError;
use super::reserved::DISALLOWED_TLDS;
use super::syntax;

/// Any syntactically valid atproto handle, unverified: the input to an identity
/// lookup, not a claim. Lowercased, with no surrounding whitespace and no
/// trailing dot; a caller trims what a person typed. Unlike
/// [`Handle`](super::Handle) it accepts `xn--` labels, `.test` and the names
/// Zurfur reserves for itself.
///
/// ```
/// use domain::elements::handle::AtHandle;
///
/// let handle: AtHandle = "Alice.Bsky.Social".parse().unwrap();
/// assert_eq!(handle.to_string(), "alice.bsky.social");
///
/// // An IDN handle is an existing identity signing in, so it is accepted.
/// assert!("xn--ls8h.example.com".parse::<AtHandle>().is_ok());
///
/// // Surrounding whitespace and a trailing dot are invalid syntax, and so is a
/// // URL or a DID.
/// assert!(" alice.bsky.social".parse::<AtHandle>().is_err());
/// assert!("alice.bsky.social.".parse::<AtHandle>().is_err());
/// assert!("https://evil.example.com".parse::<AtHandle>().is_err());
/// assert!("did:plc:z72i7hdynmk6r22z27h6tvur".parse::<AtHandle>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct AtHandle(String);

impl FromStr for AtHandle {
    type Err = HandleError;

    /// Lowercase ASCII only, then check the handle syntax and refuse the spec's
    /// disallowed TLDs. Whitespace and a trailing dot are refused, never stripped.
    ///
    /// ```
    /// use domain::elements::handle::{AtHandle, HandleError};
    ///
    /// assert_eq!("alice".parse::<AtHandle>(), Err(HandleError::TooFewSegments));
    /// assert_eq!("alice.bsky.social.".parse::<AtHandle>(), Err(HandleError::EmptySegment));
    /// assert_eq!("alice.bsky.social\n".parse::<AtHandle>(), Err(HandleError::InvalidChar('\n')));
    /// assert_eq!("laptop.local".parse::<AtHandle>(), Err(HandleError::ReservedTld("local".into())));
    /// ```
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.to_ascii_lowercase();
        let labels = syntax::split_labels(&normalized)?;
        syntax::check_top_level(&labels, &DISALLOWED_TLDS)?;
        Ok(Self(normalized))
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
