//! [`AtprotoAuthenticator`]: OAuth sign-in against the visitor's PDS, on the
//! identity resolver. `start` resolves the handle both ways, checks the
//! authorization endpoint before the Pushed Authorization Request, and binds the
//! DID and the browser to the request; `complete` lets one callback claim the
//! sign-in, refuses a callback from any other browser, a sign-in as any other
//! account and a grant other than `atproto`, and keeps no OAuth session: a
//! failed sign-in's is deleted, a finished one's is deleted, then revoked in
//! the background, and a wider grant is never stored and is revoked too.

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
    request::{OAuthMetadata, RequestError, RequestErrorKind, par},
    resolver::{OAuthResolver, ResolverError},
    scopes::Scopes,
    session::{ClientData, ClientSessionData, SessionRegistry},
    types::CallbackParams,
};
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};
use smol_str::SmolStr;
use sqlx::PgPool;

use domain::elements::{did::Did, handle::AtHandle};
use domain::ports::{
    AccountMismatch, Authenticator, BrowserBinding, BrowserMismatch, ResolveError, SigninStarted,
};

use crate::auth_store::{AtprotoAuthStore, StoredBinding};
use crate::guarded_http::{Causes, PublicHttpsUrl, UrlPolicyError};
use crate::oauth_scope::{OAUTH_SCOPES, grants_identity_only};
use crate::resolver::AtprotoIdentityResolver;
use crate::secret_vault::SecretVault;
use jacquard_bridge::JacquardBridge;

/// How long a started sign-in stays valid, and so its browser token: the
/// cookie's lifetime and the age past which the callback is refused.
const SIGNIN_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// The jacquard OAuth client sign-in drives, over the bridge and the
/// Postgres-backed store. Private, so the protocol type never leaves the crate.
type Oauth = OAuthClient<JacquardBridge, AtprotoAuthStore>;

/// A session jacquard's callback saved, as it hands it back.
type Session = OAuthSession<JacquardBridge, AtprotoAuthStore>;

/// The registry jacquard's client and sessions share: the store and this
/// client's own data.
type Registry = SessionRegistry<JacquardBridge, AtprotoAuthStore, SmolStr>;

/// How long each part of the handshake may take, every lookup and request in
/// it included.
#[derive(Clone, Copy, Debug)]
struct Deadlines {
    /// `start`: the both-ways resolution, two metadata fetches and the PAR.
    start: Duration,
    /// `complete`'s callback: the issuer metadata, the token exchange and the
    /// issuer check. The cleanup after it runs outside this deadline.
    complete: Duration,
    /// Revoking a sign-in's tokens, after its session is deleted.
    revoke: Duration,
    /// Deleting a sign-in's session, so a stuck statement cannot hold the
    /// callback's task.
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

/// How a callback that ran to the end turned out. Each outcome carries the
/// grant it got, which is deleted and revoked.
enum Completed {
    /// The bound account signed in with exactly `atproto`.
    SignedIn { did: Did, grant: Box<Grant> },
    /// Another account signed in.
    OtherAccount(Box<Grant>),
    /// The bound account signed in, but with a grant other than exactly `atproto`.
    RefusedScope(Box<Grant>),
}

/// A grant a callback got: its tokens and DPoP key, and the client to revoke
/// them through.
struct Grant {
    client: Arc<JacquardBridge>,
    registry: Arc<Registry>,
    data: ClientSessionData,
}

/// The grant behind a session jacquard's callback saved.
impl From<Session> for Grant {
    fn from(session: Session) -> Self {
        Self {
            client: session.client,
            registry: session.registry,
            data: session.data.into_inner(),
        }
    }
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

    /// Run the callback under its deadline, then, outside it, delete what the
    /// claimer saved and revoke any grant it got: in the background after a
    /// sign-in or a refused scope, before the answer for another account. An
    /// attempt that never claimed deletes nothing.
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
            Ok(Completed::SignedIn { did, grant }) => {
                // A failed delete is logged and does not fail the sign-in.
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                tokio::spawn(revoke(grant, deadlines.revoke));
                return Ok(did);
            }
            Ok(Completed::OtherAccount(grant)) => {
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                revoke(grant, deadlines.revoke).await;
                SigninFailure::AccountMismatch
            }
            Ok(Completed::RefusedScope(grant)) => {
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                tokio::spawn(revoke(grant, deadlines.revoke));
                SigninFailure::GrantedScope
            }
            // Only the claimer runs the callback, so every session stored
            // under `state` is its own.
            Err(failure) if claimed => {
                self.discard(session_id.as_deref(), deadlines.cleanup).await;
                // A grant the store refused is held for this claimer alone; a
                // deadline may have cut `finish` off before it took it.
                if let Some(refused) = self.take_refused(session_id.as_deref()) {
                    tokio::spawn(revoke(refused, deadlines.revoke));
                }
                failure
            }
            Err(failure) => failure,
        };
        log_failure(&failure);
        Err(failure)
    }

    /// Claim the sign-in for this browser, run jacquard's callback and judge
    /// the grant it got, saved or refused by the store. Sets `claimed` once
    /// this attempt owns the sign-in; a claim cut off by the deadline fails
    /// closed, since no callback ran.
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
            state: Some(state.as_str().into()),
            iss: iss.map(Into::into),
        };
        let called_back = self.oauth.callback(params).await;
        let grant = match called_back {
            Ok(session) => Box::new(Grant::from(session)),
            Err(error) => self
                .take_refused(Some(&state))
                .ok_or_else(|| SigninFailure::Callback(Box::new(error)))?,
        };
        Ok(judged(grant, &bound))
    }

    /// The grant the store refused under `session_id`, if one is held, ready
    /// to revoke. Only the claimer calls this, so the grant is its own.
    fn take_refused(&self, session_id: Option<&str>) -> Option<Box<Grant>> {
        let refused = self.oauth.registry.store.take_refused(session_id?)?;
        let grant = Grant {
            client: self.oauth.client.clone(),
            registry: self.oauth.registry.clone(),
            data: refused,
        };
        Some(Box::new(grant))
    }

    /// Refuse unless `presented` is the token bound to `state`'s live request,
    /// compared by hash in constant time, then claim the sign-in. A refused
    /// request is deleted, so nothing can complete it later, unless a callback
    /// has claimed it: a claimed request, however old, is left to its claimer.
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
        // Conditional on the binding, not on the read above: a claim may have
        // committed since, and a claimed request is never a refusal's to delete.
        store
            .delete_unclaimed_auth_request(state)
            .await
            .map_err(SigninFailure::Store)?;
        Err(refusal)
    }

    /// Consume `state`'s browser binding, still `stored` and within the
    /// sign-in's lifetime, in one conditional statement: of callbacks racing on
    /// it, exactly one clears it and owns the sign-in. A loser deletes nothing,
    /// since the request is the winner's.
    async fn claim(&self, state: &str, stored: &[u8]) -> Result<(), SigninFailure> {
        let won = self
            .oauth
            .registry
            .store
            .claim_browser_binding(state, stored, SIGNIN_LIFETIME)
            .await
            .map_err(SigninFailure::Store)?;
        if !won {
            return Err(SigninFailure::Claimed);
        }
        Ok(())
    }

    /// The DID `start` bound to `state`. A request without one fails closed,
    /// and is deleted so nothing can complete it later; only the claimer gets
    /// here, so the request is its own to delete.
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
    /// session by the sign-in's `state`), then the claimed request under that
    /// `state` if jacquard has not already, within `limit`, logging a failure by
    /// class. Only the claimer calls this, so the request is its own.
    async fn discard(&self, session_id: Option<&str>, limit: Duration) {
        let Some(session_id) = session_id else {
            return;
        };
        let store = &self.oauth.registry.store;
        let deleting = async {
            store.delete_sessions_by_id(session_id).await?;
            store.delete_auth_req_info(session_id).await
        };
        let deleted = tokio::time::timeout(limit, deleting).await;
        let failure = match deleted {
            Ok(Ok(())) => return,
            Ok(Err(error)) => SigninFailure::Store(error),
            Err(_elapsed) => SigninFailure::CleanupDeadline,
        };
        log_failure(&failure);
    }
}

/// Whose account `grant` signed in and what it grants: another account is
/// judged first, whatever its grant; then a grant other than exactly
/// [`OAUTH_SCOPES`]; otherwise the bound account signed in.
fn judged(grant: Box<Grant>, bound: &AtDid) -> Completed {
    let signed_in = grant.data.account_did.as_str();
    if signed_in != bound.as_str() {
        return Completed::OtherAccount(grant);
    }
    if !grants_identity_only(grant.data.token_set.scope.as_deref()) {
        return Completed::RefusedScope(grant);
    }
    let did = Did::from(signed_in.to_owned());
    Completed::SignedIn { did, grant }
}

/// Revoke `grant`'s tokens at its authorization server, best effort and
/// within `limit`, logging a failure by class; it is no longer stored.
async fn revoke(grant: Box<Grant>, limit: Duration) {
    let mut grant = *grant;
    let revoked = tokio::time::timeout(limit, revoke_tokens(&mut grant)).await;
    if revoked.is_err() {
        log_revocation_failure(&RevocationFailure::Deadline);
    }
}

/// Revoke the refresh token, which ends the whole grant, then the access token,
/// each on its own: a failure of one is logged and the other still sent.
async fn revoke_tokens(grant: &mut Grant) {
    let bridge = grant.client.as_ref();
    let data = &mut grant.data;
    let client_data = &grant.registry.client_data;
    let metadata = OAuthMetadata::new(bridge, client_data, data).await;
    let metadata = match metadata {
        Ok(metadata) => metadata,
        Err(error) => {
            log_revocation_failure(&RevocationFailure::Metadata(Box::new(error)));
            return;
        }
    };
    if metadata.server_metadata.revocation_endpoint.is_none() {
        log_revocation_failure(&RevocationFailure::NoEndpoint);
        return;
    }
    let refresh_token = data.token_set.refresh_token.clone();
    let access_token = data.token_set.access_token.clone();
    for token in refresh_token.into_iter().chain([access_token]) {
        let sent =
            jacquard_oauth::request::revoke(bridge, &mut data.dpop_data, &token, &metadata).await;
        if let Some(failure) = revocation_failure(sent) {
            log_revocation_failure(&failure);
        }
    }
}

/// How a revocation request `sent` failed, if it did. A `200` counts as
/// revoked: RFC 7009 answers a revocation with `200`, and jacquard accepts
/// only `204`.
fn revocation_failure(sent: Result<(), RequestError>) -> Option<RevocationFailure> {
    let error = sent.err()?;
    let answered_ok = matches!(
        error.kind(),
        RequestErrorKind::HttpStatus(http::StatusCode::OK)
    );
    if answered_ok {
        return None;
    }
    Some(RevocationFailure::Request(Box::new(error)))
}

#[async_trait]
impl Authenticator for AtprotoAuthenticator {
    /// Resolve `handle` both ways, check the PDS and its authorization endpoint,
    /// run the Pushed Authorization Request with the DID and a fresh browser
    /// token bound, and return the authorization URL and that token. Every
    /// failure is one error, logged by class only.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn start(&self, handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        let started = tokio::time::timeout(self.deadlines.start, self.signin.begin(handle))
            .await
            .unwrap_or(Err(SigninFailure::Deadline));
        started.map_err(|failure| {
            log_failure(&failure);
            anyhow::Error::new(failure)
        })
    }

    /// Exchange the callback params for tokens and return the DID `start`
    /// bound. Refuses another browser ([`BrowserMismatch`], unless the sign-in
    /// is claimed), another account ([`AccountMismatch`]) and any grant but
    /// `atproto`; keeps no OAuth session, in a task a dropped request can't cut.
    #[tracing::instrument(level = "debug", skip_all)]
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
            // The port's one browser refusal, the class kept beneath it.
            refused @ (SigninFailure::NoBrowserBinding
            | SigninFailure::StaleSignin
            | SigninFailure::BrowserMismatch) => {
                anyhow::Error::new(refused).context(BrowserMismatch)
            }
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
    /// The token response granted a scope other than exactly `atproto`, or
    /// named none.
    #[error("granted scope refused")]
    GrantedScope,
    /// The callback's task panicked or was cancelled.
    #[error("sign-in task failed")]
    Task(#[source] tokio::task::JoinError),
    /// The deadline ran out.
    #[error("sign-in deadline passed")]
    Deadline,
    /// Deleting a sign-in's session ran out of time.
    #[error("cleanup deadline passed")]
    CleanupDeadline,
}

/// The failure's class at `info`; its causes, which can name a host, only at
/// `debug`, read through `source()` (a `ResolveError`'s `Debug` hides them).
fn log_failure(failure: &SigninFailure) {
    tracing::info!(failure = %failure, "sign-in failed");
    tracing::debug!(cause = %Causes(failure), "sign-in failure cause");
}

/// Why revoking a grant's tokens failed. The grant's session is already
/// deleted, and the sign-in's answer does not depend on it. The message is
/// the class only: it never names a token, the `state` or the DID.
#[derive(Debug, thiserror::Error)]
enum RevocationFailure {
    /// The authorization server's metadata could not be fetched.
    #[error("authorization server metadata failed")]
    Metadata(#[source] Box<RequestError>),
    /// The authorization server names no revocation endpoint, so the tokens
    /// are left to expire.
    #[error("no revocation endpoint")]
    NoEndpoint,
    /// A revocation request failed or was refused.
    #[error("revocation request failed")]
    Request(#[source] Box<RequestError>),
    /// Revoking ran out of time.
    #[error("revocation deadline passed")]
    Deadline,
}

/// A failed revocation's class at `info`, under its own message; its causes
/// only at `debug`.
fn log_revocation_failure(failure: &RevocationFailure) {
    tracing::info!(failure = %failure, "revocation failed");
    tracing::debug!(cause = %Causes(failure), "revocation failure cause");
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

    /// This authenticator with its session delete cut off after `cleanup`.
    fn with_cleanup(self, cleanup: Duration) -> Self {
        let deadlines = Deadlines {
            cleanup,
            ..self.deadlines
        };
        Self { deadlines, ..self }
    }
}

#[cfg(test)]
mod tests;
