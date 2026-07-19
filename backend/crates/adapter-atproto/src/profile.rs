//! Public profile reads from the visitor's own PDS — see [`AtprotoProfileSource`].
//!
//! Implements [`ProfileSource`] the decentralized way: resolve the DID document
//! for handle + PDS endpoint, then read the `app.bsky.actor.profile` record
//! straight from that PDS — no Bluesky appview, no CDN. Unauthenticated, because
//! profiles are public (ZMVP-10).

use async_trait::async_trait;
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileSource,
};
use jacquard::api::app_bsky::actor::profile::Profile as BskyProfile;
use jacquard::client::{AgentSessionExt, BasicClient};
use jacquard::common::types::collection::RecordError;
use jacquard::common::types::string::{AtUri, Did as AtDid, Handle};
use jacquard::common::xrpc::XrpcError;
use jacquard::prelude::IdentityResolver;
use smol_str::SmolStr;

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
    /// Build the source with a fresh unauthenticated jacquard client. Stateless
    /// apart from that client; [`Default`] calls this.
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
    /// Two hops, both fallible network reads:
    ///
    /// 1. Resolve the DID document (claimed handle from `alsoKnownAs`, PDS
    ///    endpoint). Errors if the DID is malformed, the document can't be
    ///    resolved, or it carries no handle / no PDS endpoint. The `alsoKnownAs`
    ///    handle is only *claimed*, so it is then **bidirectionally verified** —
    ///    the claimed handle is resolved back and must return this same DID
    ///    ([`presented_handle`]); a claim that can't be confirmed is never
    ///    presented as the actor's handle (we fall back to the DID string).
    /// 2. Read `app.bsky.actor.profile/self` from that PDS. The error mapping is
    ///    deliberate: a `RecordNotFound` is **not** an error — the visitor just
    ///    has no display name/avatar yet, so we return a handle-only [`Profile`]
    ///    the caller may safely cache. Any *other* response error (auth, server,
    ///    decode) and any failure to reach the PDS are propagated as `Err`, so a
    ///    transient fault never gets cached as a stripped profile pinned for the
    ///    TTL (ZMVP-10 criterion 1).
    ///
    /// The avatar URL is built against the user's own PDS
    /// (`com.atproto.sync.getBlob`), keeping the read on the decentralized path.
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

        // The DID document's first `alsoKnownAs` is the handle it *claims*. A DID
        // document is self-asserted, so the claim is untrusted until verified: an
        // attacker's document can name any `alsoKnownAs`. atproto trusts a handle only
        // under **bidirectional** verification — resolve the claimed handle back and
        // confirm it returns THIS DID (see [`presented_handle`]).
        let claimed_handle = doc
            .also_known_as
            .as_ref()
            .and_then(|aka| aka.first())
            .map(|aka| aka.as_str().trim_start_matches("at://").to_string())
            .ok_or_else(|| anyhow::anyhow!("DID document carries no handle"))?;

        // Resolve the claimed handle back to a DID (DNS `_atproto` / HTTPS well-known —
        // the same [`IdentityResolver`] the sign-in path trusts). A malformed handle,
        // or any failure to resolve it, leaves `resolved_back` = `None`, i.e. the claim
        // is unconfirmed and must not be presented as the actor's.
        let resolved_back = match Handle::new(SmolStr::from(claimed_handle.as_str())) {
            Ok(handle) => self.client.resolve_handle(&handle).await.ok(),
            Err(_) => None,
        };
        let handle = presented_handle(
            did.as_str(),
            &claimed_handle,
            resolved_back.as_ref().map(|resolved| resolved.as_str()),
        );

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

/// Decide which handle to present for `did`, given the `candidate` handle claimed in
/// the DID document's `alsoKnownAs` and the DID that candidate resolves **back** to
/// (`resolved_back`; `None` when the reverse resolution could not be completed at all).
///
/// atproto handles are trustworthy only under **bidirectional** verification: a DID
/// document is self-asserted and can claim any `alsoKnownAs`, so a handle is the
/// actor's only if resolving that handle (DNS / HTTPS well-known) returns THIS same
/// DID. On any failure to confirm — the reverse resolves to a *different* DID (a stale
/// or spoofed claim) or could not be resolved — we fall back to the DID string itself,
/// which is the actor's own verifiable, unspoofable identifier. We never surface an
/// unconfirmed handle as though it were the actor's confirmed one.
///
/// (The domain [`Profile::handle`](domain::elements::profile::Profile) is a required
/// `String`, so "unverified" is represented by substituting the DID rather than a
/// `None`; a first-class `verified`/`Option<handle>` distinction is a domain follow-up
/// — see the crate notes.)
fn presented_handle(did: &str, candidate: &str, resolved_back: Option<&str>) -> String {
    match resolved_back {
        Some(back) if back == did => candidate.to_string(),
        _ => did.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::presented_handle;

    const DID: &str = "did:plc:actor";

    // Finding 2: a handle is only the actor's when it resolves BACK to this DID.
    #[test]
    fn a_handle_that_resolves_back_to_this_did_is_presented() {
        assert_eq!(
            presented_handle(DID, "alice.zurfur.app", Some(DID)),
            "alice.zurfur.app",
            "a bidirectionally-verified handle is trusted"
        );
    }

    #[test]
    fn a_handle_resolving_to_another_did_is_never_presented() {
        // A spoofed / stale `alsoKnownAs`: the claimed handle belongs to someone else.
        assert_eq!(
            presented_handle(DID, "victim.zurfur.app", Some("did:plc:someoneelse")),
            DID,
            "a handle owned by a different DID falls back to the DID, never impersonates"
        );
    }

    #[test]
    fn an_unresolvable_handle_falls_back_to_the_did() {
        // Reverse resolution could not be completed (malformed handle, resolver
        // failure, …) — the claim is unconfirmed, so it must not be presented.
        assert_eq!(
            presented_handle(DID, "alice.zurfur.app", None),
            DID,
            "an unconfirmable handle is never presented as trusted"
        );
    }
}
