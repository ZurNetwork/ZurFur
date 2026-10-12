//! Public profile reads from the visitor's own PDS — see [`AtprotoProfileSource`].

use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    elements::{
        did::Did,
        profile::{DisplayHandle, Profile},
    },
    ports::{ProfileSource, ResolveError},
};
use jacquard::api::app_bsky::actor::profile::Profile as BskyProfile;
use jacquard::common::IntoStatic as _;
use jacquard::common::types::collection::{Collection, RecordError};
use jacquard::common::types::ident::AtIdentifier;
use jacquard::common::types::recordkey::{RecordKey, Rkey};
use jacquard::common::types::string::Did as AtDid;
use jacquard::common::xrpc::atproto::GetRecord;
use jacquard::common::xrpc::{Response, XrpcError, XrpcExt as _};

use crate::guarded_http::PublicHttpsUrl;
use crate::resolver::AtprotoIdentityResolver;

/// The record a profile is read from.
const PROFILE_RKEY: &str = "self";

/// The real [`ProfileSource`]: the handle from the identity resolver's
/// both-ways check, the PDS from the DID document it validated, and
/// `app.bsky.actor.profile` read from that PDS over the guarded client — no
/// appview, no CDN. Unauthenticated; nothing here touches a token.
pub struct AtprotoProfileSource {
    resolver: Arc<AtprotoIdentityResolver>,
}

impl AtprotoProfileSource {
    /// The source reading through `resolver` and its guarded client.
    pub fn new(resolver: Arc<AtprotoIdentityResolver>) -> Self {
        Self { resolver }
    }

    /// Read the profile under one port-call deadline, so the chain of
    /// requests behind it never stacks its per-request limits.
    async fn read_within_deadline(&self, did: &Did) -> Result<Profile, ProfileReadError> {
        let deadline = self.resolver.port_call_deadline();
        tokio::time::timeout(deadline, self.read(did))
            .await
            .unwrap_or(Err(ProfileReadError::Deadline))
    }

    /// Read the profile, each step's failure kept apart for the log.
    async fn read(&self, did: &Did) -> Result<Profile, ProfileReadError> {
        let (document, handle) = self
            .resolver
            .resolve_did_with_document(did)
            .await
            .map_err(ProfileReadError::Identity)?;
        let pds = document.pds().map_err(ProfileReadError::Pds)?;
        let record = self.profile_record(did, &pds).await?;

        // An unconfirmed handle is never shown as the actor's: the DID stands in.
        let shown_handle = handle.map_or_else(|| did.to_string(), |handle| handle.to_string());
        let display_name = record
            .as_ref()
            .and_then(|profile| profile.display_name.as_ref())
            .map(|name| name.as_str().to_string());
        let avatar_url = record
            .as_ref()
            .and_then(|profile| profile.avatar.as_ref())
            .map(|avatar| blob_url(&pds, did, avatar.blob().cid().as_str()));
        let profile = Profile {
            did: did.clone(),
            handle: DisplayHandle::from(shown_handle),
            display_name,
            avatar_url,
        };
        Ok(profile)
    }

    /// `app.bsky.actor.profile/self` from `pds`; `None` when the PDS says the
    /// record does not exist, an error for any other failure.
    async fn profile_record(
        &self,
        did: &Did,
        pds: &PublicHttpsUrl,
    ) -> Result<Option<BskyProfile>, ProfileReadError> {
        let request = profile_request(did)?;
        let base = fluent_uri::Uri::parse(pds.as_ref())
            .map_err(|_| ProfileReadError::Request("the PDS URL"))?;
        let response = self
            .resolver
            .http()
            .xrpc(base)
            .send(&request)
            .await
            .map_err(|error| ProfileReadError::Record(Box::new(error)))?;
        let typed: Response<<BskyProfile as Collection>::Record> = response.transmute();
        match typed.into_output() {
            Ok(output) => Ok(Some(output.value)),
            // A real absence, safe for the caller to cache.
            Err(XrpcError::Xrpc(RecordError::RecordNotFound(_))) => Ok(None),
            // Not a clean absence: an error, so nothing stripped is cached.
            Err(error) => Err(ProfileReadError::Record(Box::new(error))),
        }
    }
}

#[async_trait]
impl ProfileSource for AtprotoProfileSource {
    /// Read `did`'s profile within one port-call deadline. A missing record
    /// yields a handle-only [`Profile`]; any other failure, an identity outage
    /// or the deadline included, is an `Err`, so a transient fault is never
    /// cached as a stripped profile.
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        let profile = self.read_within_deadline(did).await;
        if let Err(failure) = &profile {
            // The class only: a cause can name the handle or the host.
            tracing::info!(failure = failure.class(), "profile read failed");
        }
        Ok(profile?)
    }
}

/// The `getRecord` request for `did`'s profile record.
fn profile_request(did: &Did) -> Result<GetRecord, ProfileReadError> {
    let repo: AtDid =
        AtDid::new_owned(did.as_ref()).map_err(|_| ProfileReadError::Request("the DID"))?;
    let rkey: Rkey =
        Rkey::new_static(PROFILE_RKEY).map_err(|_| ProfileReadError::Request("the rkey"))?;
    let request: GetRecord = GetRecord {
        cid: None,
        collection: <BskyProfile as Collection>::nsid().into_static(),
        repo: AtIdentifier::Did(repo),
        rkey: RecordKey(rkey),
    };
    Ok(request)
}

/// The avatar's URL on the validated PDS, not a CDN: built like jacquard's
/// XRPC URLs (scheme, host and path prefix kept; query and fragment dropped),
/// with the DID and CID encoded as query values.
fn blob_url(pds: &PublicHttpsUrl, did: &Did, cid: &str) -> String {
    let mut url = url::Url::from(pds.clone());
    let path = format!(
        "{}/xrpc/com.atproto.sync.getBlob",
        url.path().trim_end_matches('/')
    );
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut()
        .append_pair("did", did.as_ref())
        .append_pair("cid", cid);
    url.into()
}

/// Why a profile read failed, by step. Never echoes the DID, handle or host.
#[derive(Debug, thiserror::Error)]
enum ProfileReadError {
    /// The DID's document, or its handle's back-check, did not resolve.
    #[error("profile identity lookup failed")]
    Identity(#[source] ResolveError),
    /// The document names no PDS, or one the URL policy refuses.
    #[error("profile PDS unusable")]
    Pds(#[source] ResolveError),
    /// The request could not be built from the named part.
    #[error("profile request could not be built")]
    Request(&'static str),
    /// The PDS did not answer with the record or a clean "not found".
    #[error("profile record unreadable")]
    Record(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The whole read ran past its deadline.
    #[error("profile read deadline passed")]
    Deadline,
}

impl ProfileReadError {
    /// A fixed label for the log: the step, and for an identity step its class.
    fn class(&self) -> &'static str {
        match self {
            Self::Identity(ResolveError::NotFound) => "identity_not_found",
            Self::Identity(ResolveError::NotConfirmed) => "identity_not_confirmed",
            Self::Identity(ResolveError::Refused(_)) => "identity_refused",
            Self::Identity(ResolveError::Unavailable(_)) => "identity_unavailable",
            Self::Pds(_) => "pds_unusable",
            Self::Request(_) => "request_unbuildable",
            Self::Record(_) => "record_unreadable",
            Self::Deadline => "deadline_passed",
        }
    }
}

#[cfg(test)]
mod tests;
