//! The URL policy: which URLs the guarded client may fetch or hand to a browser.

use domain::elements::handle::RESERVED_TLDS;

/// A URL that passed the policy: `https`, port 443, no userinfo, and a domain-name
/// host of at least two labels outside the local and reserved TLDs. Parsed once;
/// the URL checked is the URL dialled.
#[derive(Clone, Debug, PartialEq, Eq, derive_more::Into)]
pub(crate) struct PublicHttpsUrl(url::Url);

/// Why a URL failed the policy. Never echoes the URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum UrlPolicyError {
    /// The text does not parse as a URL.
    #[error("not a URL")]
    NotUrl,
    /// The scheme is anything but `https`.
    #[error("scheme is not https")]
    NotHttps,
    /// An explicit port other than 443.
    #[error("port is not 443")]
    Port,
    /// A username or password in the URL.
    #[error("URL carries userinfo")]
    Userinfo,
    /// The host is an IP address, in any spelling.
    #[error("host is an IP literal")]
    IpLiteral,
    /// No host, a single label, an empty label, or a local or reserved TLD.
    #[error("host is not a public domain name")]
    LocalName,
}

impl TryFrom<url::Url> for PublicHttpsUrl {
    type Error = UrlPolicyError;

    /// Admit `url` when it passes every rule of the policy.
    fn try_from(url: url::Url) -> Result<Self, Self::Error> {
        if url.scheme() != "https" {
            return Err(UrlPolicyError::NotHttps);
        }
        // The parser already drops a port equal to the scheme's default (443).
        if url.port().is_some() {
            return Err(UrlPolicyError::Port);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(UrlPolicyError::Userinfo);
        }
        // The parser turns every IPv4 spelling (hex, octal, short, trailing dot)
        // and every bracketed IPv6 into an IP host, so only a name is a Domain.
        let host = url.host().ok_or(UrlPolicyError::LocalName)?;
        let url::Host::Domain(domain) = host else {
            return Err(UrlPolicyError::IpLiteral);
        };
        if !is_public_domain(domain) {
            return Err(UrlPolicyError::LocalName);
        }
        Ok(Self(url))
    }
}

impl AsRef<str> for PublicHttpsUrl {
    /// The URL as text, exactly as it will be dialled.
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<&str> for PublicHttpsUrl {
    type Error = UrlPolicyError;

    /// Parse `text` as a URL, then admit it through the policy.
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        let url = url::Url::parse(text).map_err(|_| UrlPolicyError::NotUrl)?;
        Self::try_from(url)
    }
}

#[cfg(test)]
impl PublicHttpsUrl {
    /// Wrap `url` without the policy: the test-only relaxed route.
    pub(crate) fn unchecked(url: url::Url) -> Self {
        Self(url)
    }
}

/// After one trailing dot is stripped: two or more non-empty labels, the last
/// not a reserved TLD. Handle lookups ask it too, so DNS and HTTPS agree.
pub(crate) fn is_public_domain(domain: &str) -> bool {
    let name = domain.strip_suffix('.').unwrap_or(domain);
    let labels: Vec<&str> = name.split('.').collect();
    let Some(tld) = labels.last() else {
        return false;
    };
    let every_label_present = labels.iter().all(|label| !label.is_empty());
    labels.len() >= 2 && every_label_present && !RESERVED_TLDS.contains(tld)
}

#[cfg(test)]
mod tests;
