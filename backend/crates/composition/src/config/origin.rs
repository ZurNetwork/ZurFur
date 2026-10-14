use std::str::FromStr;

use url::{Host, Url};

/// A scheme, a host and an optional port; nothing else.
///
/// ```
/// use composition::BareOrigin;
///
/// let origin: BareOrigin = "http://127.0.0.1:8080".parse().unwrap();
/// assert!(origin.is_of_origin(&["http://127.0.0.1"]));
/// assert!("https://zurfur.app/app".parse::<BareOrigin>().is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, derive_more::AsRef)]
pub struct BareOrigin(Url);

impl TryFrom<Url> for BareOrigin {
    type Error = BareOriginError;

    /// Accept `url` only when it has a host and carries no credentials, path,
    /// query or fragment.
    fn try_from(url: Url) -> Result<Self, Self::Error> {
        if !url.username().is_empty() || url.password().is_some() {
            return Err(BareOriginError::Credentials);
        }
        if !matches!(url.path(), "" | "/") {
            return Err(BareOriginError::Path);
        }
        if url.query().is_some() {
            return Err(BareOriginError::Query);
        }
        if url.fragment().is_some() {
            return Err(BareOriginError::Fragment);
        }
        if url.host().is_none() {
            return Err(BareOriginError::NoHost);
        }
        Ok(Self(url))
    }
}

impl FromStr for BareOrigin {
    type Err = BareOriginError;

    /// Parse `raw` as a URL, then accept it only as a bare origin.
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(raw)?;
        Self::try_from(url)
    }
}

impl BareOrigin {
    /// Whether the scheme and host are one of `origins`, any port. Hosts
    /// compare parsed, so `localhost` is not `127.0.0.1` and an IPv4-mapped
    /// address is not `::1`.
    pub fn is_of_origin(&self, origins: &[&str]) -> bool {
        origins
            .iter()
            .filter_map(|origin| Url::parse(origin).ok())
            .any(|origin| origin.scheme() == self.0.scheme() && origin.host() == self.0.host())
    }

    /// Whether this is `https` on a public domain name, on a port other than
    /// `0`; see [`is_public_domain_name`].
    pub fn is_public_https(&self) -> bool {
        let public_host =
            matches!(self.0.host(), Some(Host::Domain(name)) if is_public_domain_name(name));
        self.0.scheme() == "https" && public_host && self.0.port() != Some(0)
    }
}

/// Whether `name` could name a deployed host: at least two labels, none of
/// them empty (so no trailing dot), and neither `localhost` nor a name under it.
fn is_public_domain_name(name: &str) -> bool {
    let is_loopback_name = name == "localhost" || name.ends_with(".localhost");
    let labels: Vec<&str> = name.split('.').collect();
    let has_two_labels = labels.len() >= 2;
    let has_an_empty_label = labels.iter().any(|label| label.is_empty());
    has_two_labels && !has_an_empty_label && !is_loopback_name
}

/// Why a URL is not a [`BareOrigin`]. No message echoes the value, which can
/// carry credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BareOriginError {
    /// Not a URL at all; carries the parser's value-free reason.
    #[error("it is not a URL ({0})")]
    NotAUrl(#[from] url::ParseError),
    /// It carries a user name or a password.
    #[error("it carries credentials")]
    Credentials,
    /// It carries a path beyond `/`.
    #[error("it carries a path")]
    Path,
    /// It carries a query, even an empty one.
    #[error("it carries a query")]
    Query,
    /// It carries a fragment, even an empty one.
    #[error("it carries a fragment")]
    Fragment,
    /// It names no host.
    #[error("it has no host")]
    NoHost,
}

#[cfg(test)]
mod tests;
