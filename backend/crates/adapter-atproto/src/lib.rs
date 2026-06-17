use std::sync::Arc;

use fluent_uri::Uri;
use jacquard::identity::JacquardResolver;
use jacquard_oauth::{
    atproto::AtprotoClientMetadata,
    authstore::MemoryAuthStore,
    client::OAuthClient,
    error::OAuthError,
    scopes::Scopes,
    session::ClientData,
    types::{AuthorizeOptions, CallbackParams},
};
use smol_str::SmolStr;

pub type Oauth = Arc<OAuthClient<JacquardResolver<reqwest::Client>, MemoryAuthStore>>;

/// OAuth scopes requested at sign-in. `atproto` is the base AT Protocol scope; the
/// transitional `transition:generic` grant covers the legacy XRPC surface still in use.
/// This is a protocol concern owned by this adapter, not application configuration.
const OAUTH_SCOPES: &str = "atproto transition:generic";

/// Build the loopback OAuth client with `redirect_uri` registered as its sole redirect
/// target.
///
/// jacquard sends `redirect_uris[0]` in the PAR request and derives the loopback
/// `client_id` from this list, so the URI registered here is the one the PDS actually
/// redirects back to — there is no per-request override.
pub fn build_oauth(redirect_uri: Uri<String>) -> Oauth {
    let scopes = Scopes::new(SmolStr::new_static(OAUTH_SCOPES))
        .expect("valid scopes")
        .convert();
    let config = AtprotoClientMetadata::new_localhost(Some(vec![redirect_uri]), Some(scopes));
    // MemoryAuthStore is a deliberate, scoped choice for ZMVP-8 (see the ticket's
    // item G). It holds two tiers of OAuth state in-process: the in-flight auth
    // request (PKCE verifier + DPoP key, keyed by `state`) and, post-callback, the
    // session token set. ZMVP-8 only needs the DID out of `callback()` — it makes no
    // authenticated PDS calls — and "survives a reload" is satisfied by our own
    // tower-sessions Postgres session, not by this store. The in-flight tier lives
    // only for the seconds of one sign-in round-trip within a single process, which
    // a dev binary handles fine.
    //
    // A persistent `ClientAuthStore` (Postgres) becomes REQUIRED before either: the
    // first authenticated PDS write (needs the token set + DPoP key to survive), or
    // running more than one replica (a /signin and /signin-callback handled by
    // different processes miss each other here, identical to a restart).
    Arc::new(OAuthClient::new(
        MemoryAuthStore::new(),
        ClientData {
            keyset: None,
            config,
        },
        reqwest::Client::new(),
    ))
}

pub async fn get_oauth_url(oauth: &Oauth, handle: &str) -> Result<String, OAuthError> {
    oauth
        .start_auth(handle, AuthorizeOptions::<jacquard::DefaultStr>::default())
        .await
}

/// Completes the OAuth callback and returns the authenticated visitor's DID as a
/// plain string. Takes the neutral query fields rather than jacquard's
/// `CallbackParams` so the protocol library stays quarantined behind this port; the
/// caller never touches a jacquard type.
pub async fn complete_callback(
    oauth: &Oauth,
    code: String,
    state: Option<String>,
    iss: Option<String>,
) -> Result<String, OAuthError> {
    let params = CallbackParams {
        code: code.into(),
        state: state.map(Into::into),
        iss: iss.map(Into::into),
    };
    let session = oauth.callback(params).await?;
    let did = session.data.read().await.account_did.clone();

    Ok(did.to_string())
}
