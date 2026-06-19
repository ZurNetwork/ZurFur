use async_trait::async_trait;
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileSource,
};
use jacquard::api::app_bsky::actor::profile::Profile as BskyProfile;
use jacquard::client::{AgentSessionExt, BasicClient};
use jacquard::common::types::collection::RecordError;
use jacquard::common::types::string::{AtUri, Did as AtDid};
use jacquard::common::xrpc::XrpcError;
use jacquard::prelude::IdentityResolver;

/// The real [`ProfileSource`]: reads a visitor's public profile from their own
/// PDS — the decentralized path. It resolves the DID document for the handle and
/// PDS endpoint, then reads the `app.bsky.actor.profile` record from that PDS.
/// Unauthenticated, because profiles are public; nothing here touches a token.
pub struct AtprotoProfileSource {
    client: BasicClient,
}

impl Default for AtprotoProfileSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AtprotoProfileSource {
    pub fn new() -> Self {
        // Default unauthenticated client: did:plc documents resolve through the PLC
        // directory and the record is read from the user's own PDS — no appview.
        Self {
            client: BasicClient::unauthenticated(),
        }
    }
}

#[async_trait]
impl ProfileSource for AtprotoProfileSource {
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        let at_did: AtDid = AtDid::new_owned(did.as_str())
            .map_err(|e| anyhow::anyhow!("invalid DID {}: {e:?}", did.as_str()))?;

        // 1. Resolve the DID document → handle (from `alsoKnownAs`) + PDS endpoint.
        //    Both are user-owned identity facts from the decentralized identity layer.
        let doc = self
            .client
            .resolve_did_doc_owned(&at_did)
            .await
            .map_err(|e| anyhow::anyhow!("resolving DID document: {e}"))?;

        let handle = doc
            .also_known_as
            .as_ref()
            .and_then(|aka| aka.first())
            .map(|aka| aka.as_str().trim_start_matches("at://").to_string())
            .ok_or_else(|| anyhow::anyhow!("DID document carries no handle"))?;

        let pds = doc
            .pds_endpoint()
            .ok_or_else(|| anyhow::anyhow!("DID document carries no PDS endpoint"))?;

        // 2. Read the profile record from that PDS. An absent record means the
        //    visitor simply has no display name or avatar yet — not an error; the
        //    handle alone is a valid profile (graceful degradation).
        let uri: AtUri =
            AtUri::new_owned(format!("at://{}/app.bsky.actor.profile/self", did.as_str()))
                .map_err(|e| anyhow::anyhow!("building profile AT-URI: {e:?}"))?;
        let record: Option<BskyProfile> = match self.client.get_record::<BskyProfile, _>(&uri).await
        {
            // The PDS answered.
            Ok(resp) => match resp.into_output() {
                Ok(output) => Some(output.value),
                // Genuinely no profile record yet: handle-only is correct, and the
                // caller may cache it (the absence is a real, stable fact).
                Err(XrpcError::Xrpc(RecordError::RecordNotFound(_))) => None,
                // Any other response-level error (auth, server error, decode) is not
                // a clean "absent" — surface it so the caller degrades *without*
                // caching a stripped profile that would then pin for the TTL.
                Err(e) => return Err(anyhow::anyhow!("reading profile record: {e}")),
            },
            // Couldn't reach the PDS at all — transient; propagate, don't cache.
            Err(e) => return Err(anyhow::anyhow!("reaching PDS for profile record: {e}")),
        };

        let display_name = record
            .as_ref()
            .and_then(|p| p.display_name.as_ref())
            .map(|name| name.as_str().to_string());

        // Build the avatar URL against the user's own PDS (com.atproto.sync.getBlob),
        // staying on the decentralized path rather than a CDN.
        let avatar_url = record
            .as_ref()
            .and_then(|p| p.avatar.as_ref())
            .map(|avatar| {
                format!(
                    "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
                    pds.as_str().trim_end_matches('/'),
                    did.as_str(),
                    avatar.blob().cid().as_str(),
                )
            });

        Ok(Profile {
            did: did.clone(),
            handle,
            display_name,
            avatar_url,
        })
    }
}
