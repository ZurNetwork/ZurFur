use std::sync::Arc;

use async_trait::async_trait;
use domain::{elements::did::Did, ports::Authenticator};
use fluent_uri::Uri;
use jacquard::identity::JacquardResolver;
use jacquard_oauth::{
    atproto::AtprotoClientMetadata,
    client::OAuthClient,
    scopes::Scopes,
    session::ClientData,
    types::{AuthorizeOptions, CallbackParams},
};
use smol_str::SmolStr;
use sqlx::PgPool;

mod auth_store;
mod profile;
pub use auth_store::AtprotoAuthStore;
pub use profile::AtprotoProfileSource;

type Oauth = Arc<OAuthClient<JacquardResolver<reqwest::Client>, AtprotoAuthStore>>;

/// The real [`Authenticator`]: an OAuth client that talks to the visitor's PDS.
/// Holds jacquard's concrete client so nothing protocol-shaped leaks past this
/// crate — the rest of the system depends only on the `Authenticator` port.
pub struct AtprotoAuthenticator {
    oauth: Oauth,
}

impl AtprotoAuthenticator {
    /// Build the loopback OAuth authenticator with `redirect_uri` as its sole
    /// registered redirect target (see [`build_oauth`] for why that is fixed here).
    /// `pool` backs the persistent [`AtprotoAuthStore`] and is injected by `api`.
    pub fn new(redirect_uri: Uri<String>, pool: PgPool) -> Self {
        Self {
            oauth: build_oauth(redirect_uri, AtprotoAuthStore::new(pool)),
        }
    }
}

#[async_trait]
impl Authenticator for AtprotoAuthenticator {
    async fn start(&self, handle: &str) -> anyhow::Result<String> {
        Ok(self
            .oauth
            .start_auth(handle, AuthorizeOptions::<jacquard::DefaultStr>::default())
            .await?)
    }

    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
    ) -> anyhow::Result<Did> {
        let params = CallbackParams {
            code: code.into(),
            state: state.map(Into::into),
            iss: iss.map(Into::into),
        };
        let session = self.oauth.callback(params).await?;
        let did = session.data.read().await.account_did.clone();
        Ok(Did::new(did.to_string()))
    }
}

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
fn build_oauth(redirect_uri: Uri<String>, store: AtprotoAuthStore) -> Oauth {
    let scopes = Scopes::new(SmolStr::new_static(OAUTH_SCOPES))
        .expect("valid scopes")
        .convert();
    let config = AtprotoClientMetadata::new_localhost(Some(vec![redirect_uri]), Some(scopes));
    // The OAuth store holds two tiers of state: the in-flight auth request (PKCE
    // verifier + DPoP key, keyed by `state`) and, post-callback, the session token
    // set + DPoP key. Backing it with Postgres (`AtprotoAuthStore`) instead of an
    // in-process map is what lets a grant survive a restart, lets `/signin` and
    // `/signin-callback` land on different replicas, and gives jacquard's refresh
    // machinery a durable place to write rotated tokens (ZMVP-12).
    Arc::new(OAuthClient::new(
        store,
        ClientData {
            keyset: None,
            config,
        },
        reqwest::Client::new(),
    ))
}
