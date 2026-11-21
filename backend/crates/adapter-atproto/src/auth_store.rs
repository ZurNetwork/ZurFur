//! Postgres persistence for jacquard's OAuth state — see [`AtprotoAuthStore`].
//!
//! Every `data` blob is sealed at rest under the sign-in-secrets key before it
//! is written and opened on read, bound to its row's key; the rows never hold
//! secrets in the clear. A session granting more than `atproto` is never
//! written: it is held in memory for the sign-in that got it, to revoke.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::oauth_scope::grants_identity_only;
use crate::queries::auth_store as sql;
use domain::elements::custody::{
    CustodyError, CustodySubject, ForSigninSecrets, PartsError, SealedEnvelope,
};
use domain::ports::ScopedSealer;
use jacquard_common::{bos::BosStr, session::SessionStoreError, types::did::Did};
use jacquard_oauth::{
    authstore::ClientAuthStore,
    session::{AuthRequestData, ClientSessionData},
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::PgPool;
use zeroize::Zeroizing;

/// The first part of every session blob's subject, naming its table.
const SESSION_SUBJECT: &[u8] = b"client_session";

/// The first part of every auth-request blob's subject, naming its table.
const AUTH_REQUEST_SUBJECT: &[u8] = b"auth_request";

/// What the store holds for a sign-in's browser binding.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StoredBinding {
    /// A live request, still bound to this token hash.
    Bound(Vec<u8>),
    /// A live request without a binding: a callback has claimed it. Requests
    /// are saved bound, so one saved without fails closed the same way.
    Claimed,
    /// No request younger than the sign-in's lifetime.
    Gone,
}

/// The row `get_browser_binding` reads: no row, a cleared binding, or a hash.
impl From<Option<Option<Vec<u8>>>> for StoredBinding {
    fn from(row: Option<Option<Vec<u8>>>) -> Self {
        match row {
            None => Self::Gone,
            Some(None) => Self::Claimed,
            Some(Some(hash)) => Self::Bound(hash),
        }
    }
}

/// Postgres-backed [`ClientAuthStore`]: durable storage for atproto OAuth state
/// in two row families — `client_session` (token set + DPoP key, keyed by DID +
/// session id) and `auth_request` (in-flight PKCE/DPoP state, keyed by `state`).
/// Values are JSON, not MessagePack: both record types use `#[serde(flatten)]`.
#[derive(Clone, Debug)]
pub struct AtprotoAuthStore {
    pool: PgPool,
    sealer: ScopedSealer<ForSigninSecrets>,
    refused: RefusedGrants,
}

/// Sessions the store refused to write because they grant more than
/// `atproto`, keyed by session id (the sign-in's `state`) until that sign-in
/// takes them to revoke. Shared by every clone of the store.
#[derive(Clone, Default)]
struct RefusedGrants(Arc<Mutex<HashMap<String, ClientSessionData>>>);

impl RefusedGrants {
    /// The held grants, locked. A panic while the lock was held leaves the
    /// map intact, so a poisoned lock is still used.
    fn held(&self) -> MutexGuard<'_, HashMap<String, ClientSessionData>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold `session` under its session id until its sign-in takes it.
    fn hold(&self, session: ClientSessionData) {
        let session_id = AsRef::<str>::as_ref(&session.session_id).to_owned();
        self.held().insert(session_id, session);
    }

    /// Take the grant held under `session_id`, if any, leaving none held.
    fn take(&self, session_id: &str) -> Option<ClientSessionData> {
        self.held().remove(session_id)
    }
}

/// Shows how many grants are held, never their tokens.
impl std::fmt::Debug for RefusedGrants {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let held = self.held().len();
        formatter
            .debug_struct("RefusedGrants")
            .field("held", &held)
            .finish()
    }
}

/// Why the store refused to write a session: its grant is not exactly
/// `atproto` (wider, narrower, or no scope at all).
#[derive(Debug, thiserror::Error)]
#[error("granted scope refused")]
pub(crate) struct GrantedScopeRefused;

impl AtprotoAuthStore {
    /// Wrap a connection `pool` and a sign-in-secrets `sealer` as an OAuth
    /// store; the sealer seals every stored blob at rest.
    pub fn new(pool: PgPool, sealer: ScopedSealer<ForSigninSecrets>) -> Self {
        Self {
            pool,
            sealer,
            refused: RefusedGrants::default(),
        }
    }

    /// Take the session refused under `session_id`, if any, leaving none held.
    pub(crate) fn take_refused(&self, session_id: &str) -> Option<ClientSessionData> {
        self.refused.take(session_id)
    }

    /// Save `request` together with the hash of the browser token that binds
    /// it, in one statement.
    pub(crate) async fn save_bound_auth_request(
        &self,
        request: &AuthRequestData,
        browser_binding: &[u8],
    ) -> Result<(), SessionStoreError> {
        let state = AsRef::<str>::as_ref(&request.state);
        let subject = auth_request_subject(state)?;
        let data = self.encode(&subject, request).await?;
        sql::save_bound_auth_request(&self.pool, state, &data, Some(browser_binding))
            .await
            .map_err(backend)?;
        Ok(())
    }

    /// The browser binding of `state`'s auth request, when that request is
    /// younger than `lifetime`: still bound to a token hash, or already claimed.
    pub(crate) async fn browser_binding(
        &self,
        state: &str,
        lifetime: std::time::Duration,
    ) -> Result<StoredBinding, SessionStoreError> {
        let lifetime_seconds = i32::try_from(lifetime.as_secs()).unwrap_or(i32::MAX);
        let row = sql::get_browser_binding(&self.pool, state, lifetime_seconds)
            .await
            .map_err(backend)?;
        Ok(StoredBinding::from(row))
    }

    /// Consume the browser binding of `state`'s auth request if it is still
    /// `browser_binding` and the request is younger than `lifetime`: `true` for
    /// the one caller whose statement cleared it, `false` once another has, the
    /// request has aged out, or it is gone.
    pub(crate) async fn claim_browser_binding(
        &self,
        state: &str,
        browser_binding: &[u8],
        lifetime: std::time::Duration,
    ) -> Result<bool, SessionStoreError> {
        let lifetime_seconds = i32::try_from(lifetime.as_secs()).unwrap_or(i32::MAX);
        let consumed =
            sql::claim_browser_binding(&self.pool, state, browser_binding, lifetime_seconds)
                .await
                .map_err(backend)?;
        Ok(consumed == 1)
    }

    /// Delete `state`'s auth request unless a callback has claimed it: a refused
    /// callback's cleanup, which leaves a claimed request to its claimer.
    pub(crate) async fn delete_unclaimed_auth_request(
        &self,
        state: &str,
    ) -> Result<(), SessionStoreError> {
        sql::delete_unclaimed_auth_request(&self.pool, state)
            .await
            .map_err(backend)?;
        Ok(())
    }

    /// Delete every session stored under `session_id`, whatever its account:
    /// the cleanup after a sign-in's callback saved one, failed or not.
    pub(crate) async fn delete_sessions_by_id(
        &self,
        session_id: &str,
    ) -> Result<(), SessionStoreError> {
        sql::delete_sessions_by_id(&self.pool, session_id)
            .await
            .map_err(backend)?;
        Ok(())
    }

    /// JSON-encode `value` and seal it to `subject`, so only an envelope reaches
    /// the column. The transient plaintext is zeroized once sealed.
    async fn encode<T: Serialize>(
        &self,
        subject: &CustodySubject,
        value: &T,
    ) -> Result<Vec<u8>, SessionStoreError> {
        let plaintext = Zeroizing::new(serde_json::to_vec(value)?);
        let envelope = self
            .sealer
            .seal(subject, &plaintext)
            .await
            .map_err(custody_error)?;
        Ok(envelope.as_ref().to_vec())
    }

    /// Open a stored blob sealed to `subject` and JSON-decode it. Fails closed:
    /// a blob that doesn't open under this key and `subject` is an error, never
    /// plaintext and never "no session".
    async fn decode<T: DeserializeOwned>(
        &self,
        subject: &CustodySubject,
        data: Vec<u8>,
    ) -> Result<T, SessionStoreError> {
        let envelope = SealedEnvelope::from(data);
        let opened = self
            .sealer
            .open(subject, &envelope)
            .await
            .map_err(custody_error)?;
        serde_json::from_slice(opened.expose()).map_err(decode_error)
    }
}

/// The subject a session's blob is sealed to: its table, its DID and its
/// session id.
fn session_subject(
    account_did: &str,
    session_id: &str,
) -> Result<CustodySubject, SessionStoreError> {
    let parts = [
        SESSION_SUBJECT,
        account_did.as_bytes(),
        session_id.as_bytes(),
    ];
    CustodySubject::try_from(parts.as_slice()).map_err(subject_error)
}

/// The subject an auth request's blob is sealed to: its table and its `state`.
fn auth_request_subject(state: &str) -> Result<CustodySubject, SessionStoreError> {
    let parts = [AUTH_REQUEST_SUBJECT, state.as_bytes()];
    CustodySubject::try_from(parts.as_slice()).map_err(subject_error)
}

#[cfg(test)]
impl AtprotoAuthStore {
    /// Whether a refused session is held under `session_id`; takes nothing.
    pub(crate) fn holds_refused(&self, session_id: &str) -> bool {
        self.refused.held().contains_key(session_id)
    }
}

/// Map a sqlx error into jacquard's [`SessionStoreError`], so a database fault
/// is never misread as "no session".
fn backend(e: sqlx::Error) -> SessionStoreError {
    SessionStoreError::Other(Box::new(e))
}

/// Map a seal/open failure into jacquard's [`SessionStoreError`]: an error,
/// never a `None`, so an unreadable blob fails closed. Its text names the kind
/// only.
fn custody_error(error: CustodyError) -> SessionStoreError {
    SessionStoreError::Other(Box::new(error))
}

/// Map a subject that can't be encoded into jacquard's [`SessionStoreError`].
fn subject_error(error: PartsError) -> SessionStoreError {
    SessionStoreError::Other(Box::new(error))
}

/// Why a stored sign-in blob that opened was refused: it doesn't decode as the
/// type read. Carries nothing of the blob, not even the decoder's error.
#[derive(Debug, thiserror::Error)]
#[error("a stored sign-in blob doesn't decode")]
struct UndecodableBlob;

/// Map a blob that opened but doesn't decode into jacquard's
/// [`SessionStoreError`], dropping the decoder's error, whose text quotes the
/// value it rejected.
fn decode_error(_error: serde_json::Error) -> SessionStoreError {
    SessionStoreError::Other(Box::new(UndecodableBlob))
}

impl ClientAuthStore for AtprotoAuthStore {
    async fn get_session<D: BosStr + Send + Sync>(
        &self,
        did: &Did<D>,
        session_id: &str,
    ) -> Result<Option<ClientSessionData>, SessionStoreError> {
        let data = sql::get_session(&self.pool, did.as_ref(), session_id)
            .await
            .map_err(backend)?;
        let Some(data) = data else {
            return Ok(None);
        };
        let subject = session_subject(did.as_ref(), session_id)?;
        let session = self.decode(&subject, data).await?;
        Ok(Some(session))
    }

    /// Insert-or-replace the session keyed by (DID, session id), so a rotated
    /// token set overwrites the prior row. A session whose grant is not exactly
    /// `atproto` is refused and never written; it is held for its sign-in.
    async fn upsert_session(&self, session: ClientSessionData) -> Result<(), SessionStoreError> {
        if !grants_identity_only(session.token_set.scope.as_deref()) {
            self.refused.hold(session);
            return Err(SessionStoreError::Other(Box::new(GrantedScopeRefused)));
        }
        let account_did = session.account_did.as_ref();
        let session_id = AsRef::<str>::as_ref(&session.session_id);
        let subject = session_subject(account_did, session_id)?;
        let data = self.encode(&subject, &session).await?;
        sql::upsert_session(&self.pool, account_did, session_id, &data)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn delete_session<D: BosStr + Send + Sync>(
        &self,
        did: &Did<D>,
        session_id: &str,
    ) -> Result<(), SessionStoreError> {
        sql::delete_session(&self.pool, did.as_ref(), session_id)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn get_auth_req_info(
        &self,
        state: &str,
    ) -> Result<Option<AuthRequestData>, SessionStoreError> {
        let data = sql::get_auth_req_info(&self.pool, state)
            .await
            .map_err(backend)?;
        let Some(data) = data else {
            return Ok(None);
        };
        let subject = auth_request_subject(state)?;
        let request = self.decode(&subject, data).await?;
        Ok(Some(request))
    }

    async fn save_auth_req_info(
        &self,
        auth_req_info: &AuthRequestData,
    ) -> Result<(), SessionStoreError> {
        let state = AsRef::<str>::as_ref(&auth_req_info.state);
        let subject = auth_request_subject(state)?;
        let data = self.encode(&subject, auth_req_info).await?;
        sql::save_auth_req_info(&self.pool, state, &data)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn delete_auth_req_info(&self, state: &str) -> Result<(), SessionStoreError> {
        sql::delete_auth_req_info(&self.pool, state)
            .await
            .map_err(backend)?;
        Ok(())
    }

    // `list_session_keys` keeps the trait default (empty): this store is not
    // enumerated — every lookup is keyed by DID + session id.
}
