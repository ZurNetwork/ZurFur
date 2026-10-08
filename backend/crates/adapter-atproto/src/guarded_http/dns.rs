//! The DNS filter: the guarded client's only way from a host name to an address.

use std::{net::IpAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use hickory_resolver::{TokioResolver, net::NetError};

use super::{
    address::{ForbiddenAddress, PublicAddress},
    errors::BoxError,
};

/// Why a lookup produced no address.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LookupError {
    /// The name exists nowhere, or has no A/AAAA records.
    #[error("no records")]
    NoRecords,
    /// The lookup itself failed.
    #[error("lookup failed")]
    Failed(#[source] BoxError),
}

/// A source of A/AAAA answers: the system's DNS, or a test's script.
#[async_trait]
pub(crate) trait AddressLookup: Send + Sync {
    /// Every address the fully qualified `fqdn` answers with.
    async fn lookup_ip(&self, fqdn: &str) -> Result<Vec<IpAddr>, LookupError>;
}

/// The system's DNS configuration, through one hickory resolver.
pub(crate) struct SystemDns(TokioResolver);

impl SystemDns {
    /// Build the resolver from the system configuration (`/etc/resolv.conf` on
    /// Unix); errors when it cannot be read. Inherent: it reads the system, it
    /// converts nothing.
    pub(crate) fn from_system_conf() -> Result<Self, NetError> {
        let resolver = TokioResolver::builder_tokio()?.build()?;
        Ok(Self(resolver))
    }
}

#[async_trait]
impl AddressLookup for SystemDns {
    async fn lookup_ip(&self, fqdn: &str) -> Result<Vec<IpAddr>, LookupError> {
        let answers = self.0.lookup_ip(fqdn).await.map_err(|error| {
            if error.is_no_records_found() {
                LookupError::NoRecords
            } else {
                LookupError::Failed(Box::new(error))
            }
        })?;
        Ok(answers.iter().collect())
    }
}

/// Why the filter gave the client no address. Never echoes the name.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DnsError {
    /// At least one answer is a forbidden address, so the whole name is refused.
    #[error("name resolves to a forbidden address")]
    Refused(#[from] ForbiddenAddress),
    /// No answer at all.
    #[error("name not found")]
    NotFound,
    /// The lookup failed or ran past its timeout.
    #[error("DNS unavailable")]
    Unavailable(#[source] BoxError),
}

impl From<LookupError> for DnsError {
    fn from(error: LookupError) -> Self {
        match error {
            LookupError::NoRecords => Self::NotFound,
            LookupError::Failed(cause) => Self::Unavailable(cause),
        }
    }
}

/// Resolves a host to public addresses only, under one outer timeout.
#[derive(Clone)]
pub(crate) struct FilteringDns {
    lookup: Arc<dyn AddressLookup>,
    timeout: Duration,
}

impl FilteringDns {
    /// A filter over `lookup`, each lookup bounded by `timeout`. Inherent: it
    /// assembles two parts, it converts nothing.
    pub(crate) fn new(lookup: Arc<dyn AddressLookup>, timeout: Duration) -> Self {
        Self { lookup, timeout }
    }

    /// Every address `host` resolves to, asked fully qualified so no search
    /// domain applies. Refused when any answer is forbidden.
    pub(crate) async fn public_addresses(
        &self,
        host: &str,
    ) -> Result<Vec<PublicAddress>, DnsError> {
        let fqdn = fully_qualified(host);
        let answers = tokio::time::timeout(self.timeout, self.lookup.lookup_ip(&fqdn))
            .await
            .map_err(|elapsed| DnsError::Unavailable(Box::new(elapsed)))??;
        if answers.is_empty() {
            return Err(DnsError::NotFound);
        }
        let public = answers
            .into_iter()
            .map(PublicAddress::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(public)
    }
}

/// `host` with exactly one trailing dot.
fn fully_qualified(host: &str) -> String {
    if host.ends_with('.') {
        host.to_owned()
    } else {
        format!("{host}.")
    }
}

#[cfg(test)]
mod tests;
