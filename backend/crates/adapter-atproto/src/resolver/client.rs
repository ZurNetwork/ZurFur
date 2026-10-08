//! [`AtprotoIdentityResolver`]: the port's both-ways rules over the system DNS
//! and the guarded client.

use std::{future::Future, sync::Arc, time::Duration};

use anyhow::Context as _;
use async_trait::async_trait;

use domain::elements::{did::Did, handle::AtHandle};
use domain::ports::{IdentityResolver, ResolveError};

use super::document::{Claim, ResolvedDocument};
use super::limits::Deadlines;
use crate::guarded_http::{Causes, GuardedHttp, SystemDns, TxtLookup};

/// The real [`IdentityResolver`]: DNS TXT and the HTTPS well-known file for
/// handles, its own fetch for DID documents, and both directions checked. No
/// third-party fallback, no cache.
pub struct AtprotoIdentityResolver {
    /// Every HTTPS request, and the A/AAAA half of the system DNS.
    pub(super) http: GuardedHttp,
    /// The TXT half of the same system DNS.
    pub(super) txt: Arc<dyn TxtLookup>,
    pub(super) deadlines: Deadlines,
}

impl AtprotoIdentityResolver {
    /// The resolver over the system's DNS configuration, which its guarded
    /// client shares. Errors when that configuration cannot be read or TLS
    /// cannot start. Inherent: it reads the system, it converts nothing.
    pub fn new() -> anyhow::Result<Self> {
        let system =
            SystemDns::from_system_conf().context("reading the system DNS configuration")?;
        let system = Arc::new(system);
        let http = GuardedHttp::new(system.clone())?;
        Ok(Self {
            http,
            txt: system,
            deadlines: Deadlines::default(),
        })
    }

    /// The guarded client this resolver fetches through, for the adapters that
    /// still drive jacquard over it.
    pub(crate) fn http(&self) -> &GuardedHttp {
        &self.http
    }

    /// The DID `handle` names and that DID's document, when the document
    /// claims the handle back; `NotConfirmed` when it does not.
    pub(crate) async fn confirm_handle(
        &self,
        handle: &AtHandle,
    ) -> Result<(Did, ResolvedDocument), ResolveError> {
        let did = self.did_named_by(handle).await?;
        let document = self.document(&did).await?;
        if document.claim() != Claim::Handle(handle.clone()) {
            return Err(ResolveError::NotConfirmed);
        }
        Ok((did, document))
    }

    /// `did`'s checked document and [`resolve_did`](IdentityResolver::resolve_did)'s
    /// answer from it, in one pass under the port-call deadline: one fetch
    /// for a caller that needs both, such as the profile read.
    pub(crate) async fn resolve_did_with_document(
        &self,
        did: &Did,
    ) -> Result<(ResolvedDocument, Option<AtHandle>), ResolveError> {
        self.within_deadline(self.document_and_handle(did)).await
    }

    /// The longest one port call may run; a caller chaining more requests on
    /// a resolver answer bounds its whole chain by it.
    pub(crate) fn port_call_deadline(&self) -> Duration {
        self.deadlines.port_call
    }

    /// `did`'s document, and the handle it claims when that handle names `did`
    /// back. A claim that fails the check is `None`, unless the check could
    /// not finish.
    async fn document_and_handle(
        &self,
        did: &Did,
    ) -> Result<(ResolvedDocument, Option<AtHandle>), ResolveError> {
        let document = self.document(did).await?;
        let Claim::Handle(claimed) = document.claim() else {
            return Ok((document, None));
        };
        let handle = match self.did_named_by(&claimed).await {
            Ok(named) if named == *did => Some(claimed),
            Ok(_) => None,
            Err(ResolveError::Unavailable(cause)) => return Err(ResolveError::Unavailable(cause)),
            Err(_) => None,
        };
        Ok((document, handle))
    }

    /// Run one port call under the port-call deadline, logging a failure by
    /// class only.
    async fn within_deadline<T>(
        &self,
        lookup: impl Future<Output = Result<T, ResolveError>>,
    ) -> Result<T, ResolveError> {
        let outcome = tokio::time::timeout(self.deadlines.port_call, lookup)
            .await
            .unwrap_or_else(|elapsed| Err(ResolveError::Unavailable(anyhow::Error::new(elapsed))));
        if let Err(error) = &outcome {
            log_failure(error);
        }
        outcome
    }
}

#[async_trait]
impl IdentityResolver for AtprotoIdentityResolver {
    async fn resolve_handle(&self, handle: &AtHandle) -> Result<Did, ResolveError> {
        let (did, _document) = self.within_deadline(self.confirm_handle(handle)).await?;
        Ok(did)
    }

    async fn resolve_did(&self, did: &Did) -> Result<Option<AtHandle>, ResolveError> {
        let (_document, handle) = self.resolve_did_with_document(did).await?;
        Ok(handle)
    }
}

/// The failure's class at `info`; its cause, which can name a host (a TLS
/// error does), only at `debug`.
fn log_failure(error: &ResolveError) {
    tracing::info!(failure = ?error, "identity lookup failed");
    tracing::debug!(cause = %Causes(error), "identity lookup failure cause");
}

#[cfg(test)]
impl AtprotoIdentityResolver {
    /// The resolver over `http` and a scripted `txt`, with test `deadlines`.
    pub(super) fn over(http: GuardedHttp, txt: Arc<dyn TxtLookup>, deadlines: Deadlines) -> Self {
        Self {
            http,
            txt,
            deadlines,
        }
    }

    /// The resolver over `http` and a scripted `txt`, with the production
    /// deadlines: the seam the sign-in and profile tests build on.
    pub(crate) fn scripted(http: GuardedHttp, txt: Arc<dyn TxtLookup>) -> Self {
        Self::over(http, txt, Deadlines::default())
    }

    /// This resolver with its port-call deadline shortened to `deadline`.
    pub(crate) fn with_port_call_deadline(mut self, deadline: Duration) -> Self {
        self.deadlines.port_call = deadline;
        self
    }
}

#[cfg(test)]
mod tests;
