#[cfg(test)]
mod tests;

use super::ChannelPointerError;
use crate::string_builder::{StringBuilder, StringBuilderViolation};

/// The commission's external linked-channel pointer — "where we talk": any URL
/// or handle, stored as raw text and rendered as an opaque pointer that never
/// auto-embeds, so no scheme allowlist is applied. Enforced here: trimmed,
/// non-empty, at most [`MAX_CHARS`](Self::MAX_CHARS), no control characters.
///
/// ```
/// use domain::elements::commission::ChannelPointer;
///
/// let url = "  https://t.me/refsheet-chat  ".parse::<ChannelPointer>().unwrap();
/// assert_eq!(url.as_str(), "https://t.me/refsheet-chat"); // trimmed
///
/// "@artist on Telegram".parse::<ChannelPointer>().unwrap(); // not a URL — fine
///
/// assert!("   ".parse::<ChannelPointer>().is_err()); // empty after trim
/// assert!("x\ny".parse::<ChannelPointer>().is_err()); // control character
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelPointer(String);

impl ChannelPointer {
    /// The length cap, in characters.
    pub const MAX_CHARS: usize = 512;

    /// The validated, trimmed pointer as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ChannelPointer {
    type Error = ChannelPointerError;

    /// Validate and wrap a pointer: trim, then reject empty, over
    /// [`MAX_CHARS`](Self::MAX_CHARS), or any control character. Anything else
    /// — URL or not — is accepted.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(Self::MAX_CHARS)
            .no_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => ChannelPointerError::Empty,
                StringBuilderViolation::TooLong { .. } => ChannelPointerError::TooLong,
                StringBuilderViolation::ControlCharacter => ChannelPointerError::ControlCharacter,
            })
    }
}

impl std::str::FromStr for ChannelPointer {
    type Err = ChannelPointerError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for ChannelPointer {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
