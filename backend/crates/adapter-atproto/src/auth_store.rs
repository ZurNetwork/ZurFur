//! Postgres persistence for jacquard's OAuth state — see [`AtprotoAuthStore`].
//!
//! Every `data` blob is sealed at rest ([`SecretVault`]) before it is written
//! and opened on read; the rows never hold secrets in the clear.

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

/// Postgres-backed [`ClientAuthStore`]: durable storage for atproto OAuth state
/// in two row families — `client_session` (token set + DPoP key, keyed by DID +
/// session id) and `auth_request` (in-flight PKCE/DPoP state, keyed by `state`).
/// Values are JSON, not MessagePack: both record types use `#[serde(flatten)]`.
#[derive(Clone, Debug)]
pub struct AtprotoAuthStore {
    pool: PgPool,
    vault: SecretVault,
}

impl AtprotoAuthStore {
    /// Wrap a connection `pool` and a [`SecretVault`] as an OAuth store; the
    /// vault seals every stored blob at rest.
    pub fn new(pool: PgPool, vault: SecretVault) -> Self {
        Self { pool, vault }
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
    /// token set overwrites the prior row.
    async fn upsert_session(&self, session: ClientSessionData) -> Result<(), SessionStoreError> {
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
