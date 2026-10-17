use std::str::FromStr;

use super::{Handle, HandleDomainError};

/// The DNS namespace Zurfur issues Account handles under, as deployment
/// configures it — normalized (trimmed, ASCII-lowercased, outer dots stripped) and
/// parsed once at config load so no call site re-normalizes it. It must be a
/// bare hostname that itself parses as a [`Handle`].
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

    /// Normalize and wrap a configured handle namespace: trim, lowercase ASCII
    /// only (as a [`Handle`] does), and
    /// strip leading and trailing dots, then refuse an empty result and
    /// anything that is not a hostname a [`Handle`] could be (a wildcard, a
    /// port, a scheme, a path, an IP literal).
    ///
    /// ```
    /// use domain::elements::handle::{HandleDomain, HandleDomainError, HandleError};
    ///
    /// assert_eq!(".zurfur.app.".parse::<HandleDomain>().unwrap().as_str(), "zurfur.app");
    /// assert_eq!("".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    /// let with_port = "zurfur.app:443".parse::<HandleDomain>();
    /// assert_eq!(with_port, Err(HandleDomainError::NotAHostname(HandleError::InvalidChar(':'))));
    /// ```
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.trim().trim_matches('.').to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(HandleDomainError::Empty);
        }
        normalized
            .parse::<Handle>()
            .map_err(HandleDomainError::NotAHostname)?;
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
