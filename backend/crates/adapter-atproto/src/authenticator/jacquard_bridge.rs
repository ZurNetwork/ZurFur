//! [`JacquardBridge`]: the only resolver jacquard's OAuth client runs on. Every
//! request it makes goes through the guarded client, every DID document comes
//! from the identity resolver's checked fetch, and it never resolves a handle.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use jacquard::identity::resolver::{
    DidDocResponse, IdentityError, IdentityResolver, PlcSource, ResolverOptions,
};
use jacquard_common::{
    bos::BosStr,
    http_client::HttpClient,
    types::{did::Did as AtDid, string::Handle},
};
use jacquard_oauth::{dpop::DpopExt, resolver::OAuthResolver};
use smol_str::SmolStr;

use domain::elements::did::Did;
use domain::ports::ResolveError;

use crate::guarded_http::{FetchError, GuardedHttp};
use crate::resolver::AtprotoIdentityResolver;

/// How long jacquard's one document fetch may take: the callback's issuer
/// check, resolving the DID the authorization server named.
const DOCUMENT_DEADLINE: Duration = Duration::from_secs(20);

/// What jacquard's OAuth client is built over: HTTP through [`GuardedHttp`],
/// documents through the resolver, handles never.
pub(crate) struct JacquardBridge {
    resolver: Arc<AtprotoIdentityResolver>,
    http: GuardedHttp,
    options: ResolverOptions,
    document_deadline: Duration,
}

impl JacquardBridge {
    /// The bridge over `resolver` and its guarded client.
    pub(crate) fn new(resolver: Arc<AtprotoIdentityResolver>) -> Self {
        let http = resolver.http().clone();
        Self {
            resolver,
            http,
            options: no_fallbacks(),
            document_deadline: DOCUMENT_DEADLINE,
        }
    }
}

/// Options for any jacquard default method that reads them: no handle steps,
/// no document steps, no PDS fallback and no Bluesky fallback.
fn no_fallbacks() -> ResolverOptions {
    ResolverOptions {
        plc_source: PlcSource::default(),
        pds_fallback: None,
        handle_order: Vec::new(),
        did_order: Vec::new(),
        validate_doc_id: true,
        public_fallback_for_handle: false,
        request_timeout: None,
    }
}

/// Why the bridge refused a handle: jacquard is never handed one, so a call is a bug.
#[derive(Debug, thiserror::Error)]
#[error("handle resolution is not jacquard's")]
pub(crate) struct HandleTripwire;

impl HttpClient for JacquardBridge {
    type Error = FetchError;

    /// Send through the guarded client.
    async fn send_http(
        &self,
        request: http::Request<Vec<u8>>,
    ) -> Result<http::Response<Vec<u8>>, Self::Error> {
        self.http.send_http(request).await
    }
}

impl IdentityResolver for JacquardBridge {
    fn options(&self) -> &ResolverOptions {
        &self.options
    }

    /// Always fails: handles resolve through Zurfur's resolver, before jacquard runs.
    async fn resolve_handle<S: BosStr + Sync>(
        &self,
        _handle: &Handle<S>,
    ) -> Result<AtDid, IdentityError>
    where
        Self: Sync,
    {
        tracing::error!("jacquard asked the sign-in bridge to resolve a handle");
        let tripwire = SmolStr::new_static("handle resolution refused");
        Err(IdentityError::transport(tripwire, HandleTripwire))
    }

    /// Zurfur's checked fetch: the method and shape rules, the guarded client,
    /// the strict parse and the `id` check, all before jacquard sees a byte.
    async fn resolve_did_doc<S: BosStr + Sync>(
        &self,
        did: &AtDid<S>,
    ) -> Result<DidDocResponse, IdentityError>
    where
        Self: Sync,
    {
        let requested = Did::from(did.as_str().to_owned());
        let fetched =
            tokio::time::timeout(self.document_deadline, self.resolver.document(&requested))
                .await
                .unwrap_or_else(|elapsed| {
                    Err(ResolveError::Unavailable(anyhow::Error::new(elapsed)))
                });
        let document = fetched.map_err(|error| {
            tracing::info!(failure = ?error, "sign-in document lookup failed");
            let failed = SmolStr::new_static("DID document lookup failed");
            IdentityError::transport(failed, error)
        })?;
        let requested = AtDid::new_owned(did.as_str())
            .map_err(|error| IdentityError::transport(SmolStr::new_static("DID"), error))?;
        let response = DidDocResponse {
            buffer: Bytes::from(document.into_body()),
            status: http::StatusCode::OK,
            requested: Some(requested),
        };
        Ok(response)
    }
}

impl OAuthResolver for JacquardBridge {}

impl DpopExt for JacquardBridge {}

#[cfg(test)]
impl JacquardBridge {
    /// This bridge with its document fetch cut off after `limit`.
    fn with_document_deadline(self, limit: Duration) -> Self {
        Self {
            document_deadline: limit,
            ..self
        }
    }
}

#[cfg(test)]
mod tests;
