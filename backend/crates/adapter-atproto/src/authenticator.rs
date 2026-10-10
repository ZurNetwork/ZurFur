//! [`AtprotoAuthenticator`]: OAuth sign-in against the visitor's PDS, on the
//! identity resolver. `start` resolves the handle both ways, checks the
//! authorization endpoint before the Pushed Authorization Request, and binds the
//! DID and the browser to the request; `complete` lets one callback claim the
//! sign-in, refuses a callback from any other browser and a sign-in as any other
//! account, and never leaves a failed sign-in's session stored.

mod jacquard_bridge;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use fluent_uri::Uri;
use jacquard_common::{session::SessionStoreError, types::did::Did as AtDid};
use jacquard_oauth::{
    atproto::{AtprotoClientMetadata, atproto_client_metadata},
    authstore::ClientAuthStore,
    client::{OAuthClient, OAuthSession},
    error::OAuthError,
    request::{OAuthMetadata, RequestError, par},
    resolver::{OAuthResolver, ResolverError},
    scopes::Scopes,
    session::ClientData,
    types::CallbackParams,
};
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};
use smol_str::SmolStr;
use sqlx::PgPool;

use domain::elements::{did::Did, handle::AtHandle};
use domain::ports::{AccountMismatch, Authenticator, BrowserBinding, ResolveError, SigninStarted};

use crate::auth_store::{AtprotoAuthStore, StoredBinding};
use crate::guarded_http::{Causes, PublicHttpsUrl, UrlPolicyError};
use crate::resolver::AtprotoIdentityResolver;
use crate::secret_vault::SecretVault;
use jacquard_bridge::JacquardBridge;

/// OAuth scopes requested at sign-in: the base `atproto` scope plus the
/// transitional `transition:generic` grant for the legacy XRPC surface.
const OAUTH_SCOPES: &str = "atproto transition:generic";

/// How long a started sign-in stays valid, and so its browser token: the
/// cookie's lifetime and the age past which the callback is refused.
const SIGNIN_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// The jacquard OAuth client sign-in drives, over the bridge and the
/// Postgres-backed store. Private, so the protocol type never leaves the crate.
type Oauth = OAuthClient<JacquardBridge, AtprotoAuthStore>;

/// A session jacquard's callback saved, as it hands it back.
type Session = OAuthSession<JacquardBridge, AtprotoAuthStore>;

/// How long each part of the handshake may take, every lookup and request in
/// it included.
#[derive(Clone, Copy, Debug)]
struct Deadlines {
    /// `start`: the both-ways resolution, two metadata fetches and the PAR.
    start: Duration,
    /// `complete`'s callback: the issuer metadata, the token exchange and the
    /// issuer check. The cleanup after it runs outside this deadline.
    complete: Duration,
    /// Revoking another account's tokens, after its session is deleted.
    revoke: Duration,
    /// Deleting a failed sign-in's session, so a stuck statement cannot hold
    /// the callback's task.
    cleanup: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            start: Duration::from_secs(30),
            complete: Duration::from_secs(30),
            revoke: Duration::from_secs(10),
            cleanup: Duration::from_secs(5),
        }
    }
}

/// The real [`Authenticator`]: an OAuth client that talks to the visitor's PDS.
pub struct AtprotoAuthenticator {
    signin: Arc<Signin>,
    deadlines: Deadlines,
}

/// Sign-in's working parts, shared with the callback's own task.
struct Signin {
    resolver: Arc<AtprotoIdentityResolver>,
    oauth: Oauth,
}

/// How a callback that ran to the end turned out.
enum Completed {
    /// The bound account signed in.
    SignedIn(Did),
    /// Another account signed in; its saved session, to delete and revoke.
    OtherAccount(Box<Session>),
}

impl AtprotoAuthenticator {
    /// Build the loopback OAuth authenticator with `redirect_uri` as its sole
    /// registered redirect target. `pool` backs the persistent
    /// [`AtprotoAuthStore`]; `vault` seals that store's secrets at rest; every
    /// lookup and request goes through `resolver`.
    pub fn new(
        redirect_uri: Uri<String>,
        pool: PgPool,
        vault: SecretVault,
        resolver: Arc<AtprotoIdentityResolver>,
    ) -> Self {
        let store = AtprotoAuthStore::new(pool, vault);
        let bridge = JacquardBridge::new(resolver.clone());
        let signin = Signin {
            resolver,
            oauth: build_oauth(redirect_uri, store, bridge),
        };
        Self {
            signin: Arc::new(signin),
            deadlines: Deadlines::default(),
        }
    }
}

impl Signin {
    /// Resolve and bind, check the endpoint, push the request, save it bound
    /// to a fresh browser token, and return where to send the visitor.
    async fn begin(&self, handle: &AtHandle) -> Result<SigninStarted, SigninFailure> {
        let (did, document) = self
            .resolver
            .confirm_handle(handle)
            .await
            .map_err(SigninFailure::Resolve)?;
        let pds = document.pds().map_err(SigninFailure::Pds)?;
        let bridge = self.oauth.client.as_ref();
        let server_metadata = bridge
            .get_resource_server_metadata(resource_text(&pds))
            .await
            .map_err(|error| SigninFailure::Metadata(Box::new(error)))?;
        // The browser is sent here, so it passes the same policy as every fetch,
        // and before the PAR.
        let authorization_endpoint =
            PublicHttpsUrl::try_from(server_metadata.authorization_endpoint.as_str())
                .map_err(SigninFailure::AuthorizationEndpoint)?;

        let client_data = &self.oauth.registry.client_data;
        let client_metadata = atproto_client_metadata(&client_data.config, &client_data.keyset)
            .map_err(SigninFailure::ClientMetadata)?;
        let mut metadata = OAuthMetadata {
            server_metadata,
            client_metadata,
            keyset: client_data.keyset.clone(),
        };
        let login_hint = SmolStr::new(handle.as_ref());
        let mut request = par(bridge, Some(login_hint), None, &mut metadata, None)
            .await
            .map_err(|error| SigninFailure::Par(Box::new(error)))?;
        // Bind the DID: the callback must sign in as exactly this account.
        let bound = AtDid::new_owned(did.as_ref()).map_err(|_| SigninFailure::UnusableDid)?;
        request.account_did = Some(bound);
        // Bind the browser: only the hash is stored, the token goes to the browser.
        let token = new_browser_token();
        self.oauth
            .registry
            .store
            .save_bound_auth_request(&request, &token_hash(&token))
            .await
            .map_err(SigninFailure::Store)?;

        let client_id = metadata.client_metadata.client_id.as_str();
        let authorization_url =
            authorization_url(authorization_endpoint, client_id, &request.request_uri);
        let started = SigninStarted {
            authorization_url,
            browser_binding: BrowserBinding::from(token),
            lifetime: SIGNIN_LIFETIME,
        };
        Ok(started)
    }

    /// Run the callback under its deadline, then clean up outside it. Only the
    /// attempt that claimed the sign-in can have saved a session under `state`,
    /// so any failure of that attempt, its deadline included, deletes every
    /// session stored there; another account's session is deleted and its
    /// tokens then revoked. An attempt that never claimed deletes nothing: its
    /// `state` is a finished sign-in's, replayed, or another attempt's.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
        deadlines: Deadlines,
    ) -> Result<Did, SigninFailure> {
        let session_id = state.clone();
        let mut claimed = false;
        let finishing = self.finish(code, state, iss, browser_binding, &mut claimed);
        let finished = tokio::time::timeout(deadlines.complete, finishing)
            .await
            .unwrap_or(Err(SigninFailure::Deadline));
        let failure = match finished {
            Ok(Completed::SignedIn(did)) => return Ok(did),
            Ok(Completed::OtherAccount(session)) => {
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                self.revoke(session, deadlines.revoke).await;
                SigninFailure::AccountMismatch
            }
            Err(failure) if claimed => {
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                failure
            }
            Err(failure) => failure,
        };
        log_failure(&failure);
        Err(failure)
    }

    /// Claim the sign-in for this browser, read the bound DID, then run
    /// jacquard's callback (which saves the session it gets) and say whose
    /// account signed in. Sets `claimed` once this attempt owns the sign-in.
    async fn finish(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
        claimed: &mut bool,
    ) -> Result<Completed, SigninFailure> {
        let state = state.ok_or(SigninFailure::UnknownState)?;
        // Both before any code exchange; read before the callback, which
        // deletes the request.
        self.claim_browser(&state, browser_binding.as_ref()).await?;
        // No other attempt can run the callback for `state` now, so any session
        // stored under it is this attempt's.
        *claimed = true;
        let bound = self.bound_account(&state).await?;
        let params = CallbackParams {
            code: code.into(),
            state: Some(state.into()),
            iss: iss.map(Into::into),
        };
        let session = self
            .oauth
            .callback(params)
            .await
            .map_err(|error| SigninFailure::Callback(Box::new(error)))?;
        let signed_in = session.data.read().await.account_did.clone();
        if signed_in.as_str() != bound.as_str() {
            return Ok(Completed::OtherAccount(Box::new(session)));
        }
        let did = Did::from(signed_in.as_str().to_owned());
        Ok(Completed::SignedIn(did))
    }

    /// Refuse unless `presented` is the token bound to `state`'s live request,
    /// compared by hash in constant time, then claim the sign-in. A refused
    /// request is deleted, so nothing can complete it later; one another
    /// callback has claimed is left to that callback.
    async fn claim_browser(
        &self,
        state: &str,
        presented: Option<&BrowserBinding>,
    ) -> Result<(), SigninFailure> {
        let store = &self.oauth.registry.store;
        let binding = store
            .browser_binding(state, SIGNIN_LIFETIME)
            .await
            .map_err(SigninFailure::Store)?;
        let refusal = match (presented, binding) {
            (_, StoredBinding::Claimed) => return Err(SigninFailure::Claimed),
            (Some(token), StoredBinding::Bound(stored))
                if hashes_match(&token_hash(token.as_ref()), &stored) =>
            {
                return self.claim(state, &stored).await;
            }
            (None, _) => SigninFailure::NoBrowserBinding,
            (Some(_), StoredBinding::Gone) => SigninFailure::StaleSignin,
            (Some(_), StoredBinding::Bound(_)) => SigninFailure::BrowserMismatch,
        };
        store
            .delete_auth_req_info(state)
            .await
            .map_err(SigninFailure::Store)?;
        Err(refusal)
    }

    /// Consume `state`'s browser binding, still `stored`, in one conditional
    /// statement: of callbacks racing on it, exactly one clears it and owns the
    /// sign-in. A loser deletes nothing, since the request is the winner's.
    async fn claim(&self, state: &str, stored: &[u8]) -> Result<(), SigninFailure> {
        let won = self
            .oauth
            .registry
            .store
            .claim_browser_binding(state, stored)
            .await
            .map_err(SigninFailure::Store)?;
        if !won {
            return Err(SigninFailure::Claimed);
        }
        Ok(())
    }

    /// The DID `start` bound to `state`. A request without one fails closed,
    /// and is deleted so nothing can complete it later.
    async fn bound_account(&self, state: &str) -> Result<AtDid, SigninFailure> {
        let store = &self.oauth.registry.store;
        let request = store
            .get_auth_req_info(state)
            .await
            .map_err(SigninFailure::Store)?
            .ok_or(SigninFailure::UnknownState)?;
        let Some(bound) = request.account_did else {
            store
                .delete_auth_req_info(state)
                .await
                .map_err(SigninFailure::Store)?;
            return Err(SigninFailure::Unbound);
        };
        Ok(bound)
    }

    /// Delete every session stored under `session_id` (jacquard keys a new
    /// session by the sign-in's `state`) within `limit`, logging a failure by class.
    async fn discard(&self, session_id: Option<&str>, limit: Duration) {
        let Some(session_id) = session_id else {
            return;
        };
        let deleting = self.oauth.registry.store.delete_sessions_by_id(session_id);
        let deleted = tokio::time::timeout(limit, deleting).await;
        let failure = match deleted {
            Ok(Ok(())) => return,
            Ok(Err(error)) => SigninFailure::Store(error),
            Err(_elapsed) => SigninFailure::CleanupDeadline,
        };
        log_failure(&failure);
    }

    /// Revoke `session`'s tokens at its authorization server, best effort and
    /// within `limit`; its stored row is already gone.
    async fn revoke(&self, session: Box<Session>, limit: Duration) {
        let revoked = tokio::time::timeout(limit, session.logout()).await;
        let failed = match revoked {
            Ok(Ok(())) => return,
            Ok(Err(error)) => Some(Box::new(error)),
            Err(_elapsed) => None,
        };
        log_failure(&SigninFailure::Revocation(failed));
    }
}

#[async_trait]
impl Authenticator for AtprotoAuthenticator {
    /// Resolve `handle` both ways, check the PDS and its authorization endpoint,
    /// run the Pushed Authorization Request with the DID and a fresh browser
    /// token bound, and return the authorization URL and that token. Every
    /// failure is one error, logged by class only.
    async fn start(&self, handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        let started = tokio::time::timeout(self.deadlines.start, self.signin.begin(handle))
            .await
            .unwrap_or(Err(SigninFailure::Deadline));
        started.map_err(|failure| {
            log_failure(&failure);
            anyhow::Error::new(failure)
        })
    }

    /// Exchange the callback params for tokens and return the signed-in DID,
    /// which must be the DID `start` bound: another account is deleted, revoked
    /// and refused with [`AccountMismatch`]. Refused before any code exchange
    /// unless `browser_binding` is this sign-in's token and no other callback
    /// claimed the sign-in first. Runs in its own task, so a dropped request
    /// cannot cut the cleanup short.
    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        let signin = self.signin.clone();
        let deadlines = self.deadlines;
        let task = tokio::spawn(async move {
            signin
                .complete(code, state, iss, browser_binding, deadlines)
                .await
        });
        let finished = task.await.unwrap_or_else(|join| {
            let failure = SigninFailure::Task(join);
            log_failure(&failure);
            Err(failure)
        });
        finished.map_err(|failure| match failure {
            SigninFailure::AccountMismatch => anyhow::Error::new(AccountMismatch),
            other => anyhow::Error::new(other),
        })
    }
}

/// Why sign-in could not start or finish. The message is the class only: it
/// never names the handle, a host or the `state`; causes ride `source()`.
#[derive(Debug, thiserror::Error)]
enum SigninFailure {
    /// The handle did not resolve both ways; the resolver's class is shown.
    #[error("handle resolution failed: {0:?}")]
    Resolve(#[source] ResolveError),
    /// The DID document names no usable PDS; the resolver's class is shown.
    #[error("PDS refused: {0:?}")]
    Pds(#[source] ResolveError),
    /// The resolved DID is not one jacquard can carry.
    #[error("resolved DID unusable")]
    UnusableDid,
    /// The PDS's or authorization server's metadata failed.
    #[error("authorization server metadata failed")]
    Metadata(#[source] Box<ResolverError>),
    /// The authorization endpoint failed the URL policy.
    #[error("authorization endpoint refused")]
    AuthorizationEndpoint(#[source] UrlPolicyError),
    /// This client's own metadata could not be built.
    #[error("client metadata failed")]
    ClientMetadata(#[source] jacquard_oauth::atproto::Error),
    /// The Pushed Authorization Request failed.
    #[error("pushed authorization request failed")]
    Par(#[source] Box<RequestError>),
    /// The auth store failed.
    #[error("auth store failed")]
    Store(#[source] SessionStoreError),
    /// The callback named no stored auth request.
    #[error("unknown sign-in state")]
    UnknownState,
    /// The callback carried no browser token.
    #[error("no browser binding")]
    NoBrowserBinding,
    /// No live sign-in is bound under this state: unknown, expired or unbound.
    #[error("no live bound sign-in")]
    StaleSignin,
    /// The browser's token is not the one bound to this sign-in.
    #[error("browser binding mismatch")]
    BrowserMismatch,
    /// Another callback for this sign-in claimed it first.
    #[error("sign-in already claimed")]
    Claimed,
    /// The stored auth request has no bound DID.
    #[error("sign-in has no bound account")]
    Unbound,
    /// jacquard's callback (issuer, code exchange, issuer check) failed.
    #[error("callback failed")]
    Callback(#[source] Box<OAuthError>),
    /// The visitor signed in as an account other than the bound one.
    #[error("signed in as a different account")]
    AccountMismatch,
    /// Revoking another account's tokens failed or ran out of time; its
    /// session was already deleted.
    #[error("revocation failed")]
    Revocation(#[source] Option<Box<OAuthError>>),
    /// The callback's task panicked or was cancelled.
    #[error("sign-in task failed")]
    Task(#[source] tokio::task::JoinError),
    /// The deadline ran out.
    #[error("sign-in deadline passed")]
    Deadline,
    /// Deleting a failed sign-in's session ran out of time.
    #[error("cleanup deadline passed")]
    CleanupDeadline,
}

/// The failure's class at `info`; its causes, which can name a host, only at
/// `debug`, read through `source()` (a `ResolveError`'s `Debug` hides them).
fn log_failure(failure: &SigninFailure) {
    tracing::info!(failure = %failure, "sign-in failed");
    tracing::debug!(cause = %Causes(failure), "sign-in failure cause");
}

/// A fresh browser token: 256 random bits from the OS, base64url, unpadded.
fn new_browser_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The SHA-256 of a browser token: all that is stored of it.
fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

/// Whether `stored` is `presented`. Both are SHA-256 digests, so even a
/// timing difference reveals only digest bytes of the caller's own input,
/// never the token; the branch-free fold is defence in depth (`black_box` is
/// best effort). A stored value of any other length never matches.
fn hashes_match(presented: &[u8; 32], stored: &[u8]) -> bool {
    let Ok(stored) = <&[u8; 32]>::try_from(stored) else {
        return false;
    };
    let difference = presented
        .iter()
        .zip(stored)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right));
    std::hint::black_box(difference) == 0
}

/// The PDS as jacquard compares it with the protected resource's `resource`:
/// the URL without the trailing slash the parser adds to an empty path.
fn resource_text(pds: &PublicHttpsUrl) -> &str {
    let text = pds.as_ref();
    text.strip_suffix('/').unwrap_or(text)
}

/// `endpoint` with this client's id and the pushed request's URI appended to
/// its query, encoded.
fn authorization_url(endpoint: PublicHttpsUrl, client_id: &str, request_uri: &str) -> String {
    let mut url = url::Url::from(endpoint);
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("request_uri", request_uri);
    url.into()
}

/// Build the loopback OAuth client with `redirect_uri` registered as its sole
/// redirect target — jacquard derives the loopback `client_id` from that list
/// and there is no per-request override. Every request it makes goes over `bridge`.
fn build_oauth(
    redirect_uri: Uri<String>,
    store: AtprotoAuthStore,
    bridge: JacquardBridge,
) -> Oauth {
    let scopes = Scopes::new(SmolStr::new_static(OAUTH_SCOPES))
        .expect("valid scopes")
        .convert();
    let config = AtprotoClientMetadata::new_localhost(Some(vec![redirect_uri]), Some(scopes));
    let client_data = ClientData {
        keyset: None,
        config,
    };
    OAuthClient::new_from_resolver(store, bridge, client_data)
}

#[cfg(test)]
impl AtprotoAuthenticator {
    /// This authenticator with `start` and `complete`'s callback each cut off
    /// after `limit`, and a revocation after `revoke`.
    fn with_deadlines(self, limit: Duration, revoke: Duration) -> Self {
        let deadlines = Deadlines {
            start: limit,
            complete: limit,
            revoke,
            cleanup: Duration::from_secs(5),
        };
        Self { deadlines, ..self }
    }
}

#[cfg(test)]
mod tests;
