//! What a guarded fetch fails with: three classes, each a different retry story.

use std::error::Error;

use super::{address::ForbiddenAddress, dns::DnsError, policy::UrlPolicyError};

/// A boxed cause, kept for `source()` chains.
pub(crate) type BoxError = Box<dyn Error + Send + Sync>;

/// Why a guarded fetch returned no response. Never echoes the URL or host.
#[derive(Debug, thiserror::Error)]
pub(crate) enum FetchError {
    /// The host or the response broke a fetch rule; retrying cannot help.
    #[error("fetch refused")]
    Refused(#[from] Refusal),
    /// The host name has no address.
    #[error("host not found")]
    NotFound,
    /// DNS or the host did not answer in time, or the connection failed.
    #[error("host unavailable")]
    Unavailable(#[source] BoxError),
}

/// Which fetch rule was broken.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Refusal {
    /// The URL failed the policy.
    #[error("URL refused")]
    Url(#[from] UrlPolicyError),
    /// The host resolves to a forbidden address.
    #[error("forbidden address")]
    Address(#[from] ForbiddenAddress),
    /// The host answered with a redirect, which is never followed.
    #[error("redirect refused")]
    Redirect,
    /// The body is larger than the cap.
    #[error("body over the cap")]
    BodyTooLarge,
    /// The TLS handshake failed: a bad certificate or a broken TLS peer.
    #[error("TLS handshake failed")]
    Tls(#[source] BoxError),
    /// The request could not be built from what the caller passed.
    #[error("request refused")]
    Request(#[source] BoxError),
}

impl From<DnsError> for FetchError {
    fn from(error: DnsError) -> Self {
        match error {
            DnsError::Refused(forbidden) => Self::Refused(Refusal::Address(forbidden)),
            DnsError::NotFound => Self::NotFound,
            DnsError::Unavailable(cause) => Self::Unavailable(cause),
        }
    }
}
