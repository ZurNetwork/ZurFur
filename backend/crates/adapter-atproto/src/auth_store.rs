//! Postgres persistence for jacquard's OAuth state — see [`AtprotoAuthStore`].
//!
//! Every `data` blob is sealed at rest ([`SecretVault`]) before it is written
//! and opened on read; the rows never hold secrets in the clear. A session
//! granting more than `atproto` is never written: it is held in memory for the
//! sign-in that got it, to revoke.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::oauth_scope::grants_identity_only;
use crate::queries::auth_store as sql;
use crate::secret_vault::SecretVault;
use jacquard_common::{bos::BosStr, session::SessionStoreError, types::did::Did};
use jacquard_oauth::{
    authstore::ClientAuthStore,
    session::{AuthRequestData, ClientSessionData},
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::PgPool;
use zeroize::Zeroizing;

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
    vault: SecretVault,
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
    /// Wrap a connection `pool` and a [`SecretVault`] as an OAuth store; the
    /// vault seals every stored blob at rest.
    pub fn new(pool: PgPool, vault: SecretVault) -> Self {
        Self {
            pool,
            vault,
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
        let aad = Self::auth_request_aad(state);
        let data = self.encode(&aad, request)?;
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

    /// AEAD associated data binding a session blob to its `(account_did,
    /// session_id)` key. Table-name prefix domain-separates it from an
    /// auth-request AAD; the length-prefixed DID blocks a re-split into a
    /// different pair. Not stored — supplied again on read.
    fn session_aad(account_did: &str, session_id: &str) -> Vec<u8> {
        let did = account_did.as_bytes();
        let mut aad = Vec::with_capacity(64 + did.len() + session_id.len());
        aad.extend_from_slice(b"atproto_oauth.client_session\0");
        aad.extend_from_slice(&(did.len() as u64).to_le_bytes());
        aad.extend_from_slice(did);
        aad.extend_from_slice(session_id.as_bytes());
        aad
    }

    /// AEAD associated data binding an auth-request blob to its `state` key;
    /// the sole variable field, so no length prefix is needed. Not stored.
    fn auth_request_aad(state: &str) -> Vec<u8> {
        let mut aad = Vec::with_capacity(32 + state.len());
        aad.extend_from_slice(b"atproto_oauth.auth_request\0");
        aad.extend_from_slice(state.as_bytes());
        aad
    }

    /// JSON-encode `value` and seal it under `aad`, so only ciphertext reaches
    /// the column. The transient plaintext is zeroized once sealed.
    fn encode<T: Serialize>(&self, aad: &[u8], value: &T) -> Result<Vec<u8>, SessionStoreError> {
        let plaintext = Zeroizing::new(serde_json::to_vec(value)?);
        self.vault.seal(aad, &plaintext).map_err(seal_error)
    }

    /// Open a sealed blob under `aad` and JSON-decode it. Fails closed: a blob
    /// that is not valid ciphertext under this vault and `aad` errors rather
    /// than being read as plaintext.
    fn decode<T: DeserializeOwned>(&self, aad: &[u8], data: &[u8]) -> Result<T, SessionStoreError> {
        let plaintext = self.vault.open(aad, data).map_err(seal_error)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }
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

/// Map a seal/open failure into jacquard's [`SessionStoreError`] — an error,
/// never a `None`, so an unreadable blob fails closed.
fn seal_error(e: anyhow::Error) -> SessionStoreError {
    SessionStoreError::Other(e.into())
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
        let aad = Self::session_aad(did.as_ref(), session_id);
        data.map(|data| self.decode(&aad, &data)).transpose()
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
        let aad = Self::session_aad(account_did, session_id);
        let data = self.encode(&aad, &session)?;
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
        let aad = Self::auth_request_aad(state);
        data.map(|data| self.decode(&aad, &data)).transpose()
    }

    async fn save_auth_req_info(
        &self,
        auth_req_info: &AuthRequestData,
    ) -> Result<(), SessionStoreError> {
        let state = AsRef::<str>::as_ref(&auth_req_info.state);
        let aad = Self::auth_request_aad(state);
        let data = self.encode(&aad, auth_req_info)?;
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
