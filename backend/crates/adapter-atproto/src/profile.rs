//! Public profile reads from the visitor's own PDS — see [`AtprotoProfileSource`].

use async_trait::async_trait;
use domain::{
    elements::{
        did::Did,
        profile::{DisplayHandle, Profile},
    },
    ports::ProfileSource,
};
use jacquard::api::app_bsky::actor::profile::Profile as BskyProfile;
use jacquard::client::{AgentSessionExt, BasicClient};
use jacquard::common::types::collection::RecordError;
use jacquard::common::types::string::{AtUri, Did as AtDid, Handle};
use jacquard::common::xrpc::XrpcError;
use jacquard::prelude::IdentityResolver;
use smol_str::SmolStr;

/// The real [`ProfileSource`]: resolves the DID document for handle and PDS
/// endpoint, then reads `app.bsky.actor.profile` from that PDS — no appview, no
/// CDN. Unauthenticated; nothing here touches a token.
pub struct AtprotoProfileSource {
    client: BasicClient,
}

impl Default for AtprotoProfileSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AtprotoProfileSource {
    /// Build the source with a fresh unauthenticated jacquard client.
    pub fn new() -> Self {
        Self {
            client: BasicClient::unauthenticated(),
        }
    }
}

#[async_trait]
impl ProfileSource for AtprotoProfileSource {
    /// Resolve the DID document, then read `app.bsky.actor.profile/self` from
    /// the PDS it names. A missing record yields a handle-only [`Profile`]; any
    /// other read failure is an `Err`, so a transient fault is never cached as a
    /// stripped profile. The presented handle is bidirectionally verified.
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        let at_did: AtDid = AtDid::new_owned(AsRef::<str>::as_ref(did))
            .map_err(|e| anyhow::anyhow!("invalid DID {}: {e:?}", AsRef::<str>::as_ref(did)))?;

        let doc = self
            .client
            .resolve_did_doc_owned(&at_did)
            .await
            .map_err(|e| anyhow::anyhow!("resolving DID document: {e}"))?;

        // A DID document is self-asserted, so its `alsoKnownAs` is only a claim.
        let claimed_handle = doc
            .also_known_as
            .as_ref()
            .and_then(|aka| aka.first())
            .map(|aka| aka.as_str().trim_start_matches("at://").to_string())
            .ok_or_else(|| anyhow::anyhow!("DID document carries no handle"))?;

        // Resolve the claim back to a DID; any failure leaves it unconfirmed.
        let resolved_back = match Handle::new(SmolStr::from(claimed_handle.as_str())) {
            Ok(handle) => self.client.resolve_handle(&handle).await.ok(),
            Err(_) => None,
        };
        let handle = presented_handle(
            AsRef::<str>::as_ref(did),
            &claimed_handle,
            resolved_back.as_ref().map(|resolved| resolved.as_str()),
        );

        let pds = doc
            .pds_endpoint()
            .ok_or_else(|| anyhow::anyhow!("DID document carries no PDS endpoint"))?;

        let uri: AtUri = AtUri::new_owned(format!(
            "at://{}/app.bsky.actor.profile/self",
            AsRef::<str>::as_ref(did)
        ))
        .map_err(|e| anyhow::anyhow!("building profile AT-URI: {e:?}"))?;
        let record: Option<BskyProfile> = match self.client.get_record::<BskyProfile, _>(&uri).await
        {
            Ok(resp) => match resp.into_output() {
                Ok(output) => Some(output.value),
                // A real absence, safe for the caller to cache.
                Err(XrpcError::Xrpc(RecordError::RecordNotFound(_))) => None,
                // Not a clean "absent" — surface it so nothing stripped is cached.
                Err(e) => return Err(anyhow::anyhow!("reading profile record: {e}")),
            },
            Err(e) => return Err(anyhow::anyhow!("reaching PDS for profile record: {e}")),
        };

        let display_name = record
            .as_ref()
            .and_then(|p| p.display_name.as_ref())
            .map(|name| name.as_str().to_string());

        // Avatar URL against the user's own PDS, not a CDN.
        let avatar_url = record
            .as_ref()
            .and_then(|p| p.avatar.as_ref())
            .map(|avatar| {
                format!(
                    "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
                    pds.as_str().trim_end_matches('/'),
                    AsRef::<str>::as_ref(did),
                    avatar.blob().cid().as_str(),
                )
            });

        Ok(Profile {
            did: did.clone(),
            handle: DisplayHandle::from(handle),
            display_name,
            avatar_url,
        })
    }
}

/// Present `candidate` only when it resolves back to `did`; otherwise fall back
/// to the DID string. An unconfirmed handle is never shown as the actor's.
fn presented_handle(did: &str, candidate: &str, resolved_back: Option<&str>) -> String {
    match resolved_back {
        Some(back) if back == did => candidate.to_string(),
        _ => did.to_string(),
    }
}

#[cfg(test)]
mod tests;
