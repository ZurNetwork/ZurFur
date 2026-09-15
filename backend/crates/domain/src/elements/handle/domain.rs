use std::str::FromStr;

use super::HandleDomainError;

/// The DNS namespace Zurfur issues Account handles under, as deployment
/// configures it — normalized (trimmed, lowercased, outer dots stripped) and
/// parsed once at config load so no call site re-normalizes it. Only an empty
/// result is rejected; stricter DNS-label validation is an open follow-up.
///
/// ```
/// use domain::elements::handle::HandleDomain;
///
/// let domain: HandleDomain = "  Zurfur.App.  ".parse().unwrap();
/// assert_eq!(domain.as_str(), "zurfur.app");
/// assert_eq!(domain.to_string(), "zurfur.app");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleDomain(String);

impl FromStr for HandleDomain {
    type Err = HandleDomainError;

    /// Normalize and wrap a configured handle namespace: trim, lowercase, and
    /// strip leading and trailing dots. Only an empty result is rejected — an
    /// empty namespace would make every handle look like a member. Stricter
    /// DNS-label validation is an open follow-up.
    ///
    /// ```
    /// use domain::elements::handle::{HandleDomain, HandleDomainError};
    ///
    /// assert_eq!(".zurfur.app.".parse::<HandleDomain>().unwrap().as_str(), "zurfur.app");
    /// assert_eq!("".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    /// ```
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.trim().trim_matches('.').to_lowercase();
        if normalized.is_empty() {
            return Err(HandleDomainError::Empty);
        }
        Ok(Self(normalized))
    }
}

impl HandleDomain {
    /// The normalized namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for HandleDomain {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for HandleDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests;
