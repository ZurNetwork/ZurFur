//! Postgres persistence for jacquard's OAuth state — see [`AtprotoAuthStore`].
//!
//! This module implements jacquard's [`ClientAuthStore`] trait against the
//! `atproto_oauth` Postgres schema so the OAuth grant for a visitor outlives a
//! single process. It holds no protocol logic: jacquard owns refresh, expiry
//! skew, and the single-flight lock; this is just durable storage (ZMVP-12).
//!
//! The rows hold live secrets — the DPoP private signing key, the long-lived
//! refresh token, the PKCE verifier — so every `data` blob is **sealed at rest**
//! ([`SecretVault`]) before it is written and opened on read; the database never
//! holds them in the clear. See [`crate::secret_vault`] for the envelope.

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

/// Postgres-backed [`ClientAuthStore`]: the persistent home for atproto OAuth
/// state, replacing jacquard's in-memory `MemoryAuthStore` (ZMVP-12).
///
/// It owns no protocol logic — jacquard drives refresh (the `SessionRegistry`
/// does the single-flight lock, the expiry skew, and the delete-on-permanent-
/// failure). This type only makes that machinery durable: persist the session
/// and the upstream grant survives a restart or a move between replicas; on a
/// permanent refresh failure jacquard calls [`ClientAuthStore::delete_session`]
/// and the next request, finding no grant, ends honestly.
///
/// Two row families mirror the trait's two jobs: `client_session` (established
/// token sets + DPoP key, keyed by DID + session id) and `auth_request`
/// (in-flight PKCE/DPoP state, keyed by the OAuth `state`).
///
/// Values are JSON-encoded, not MessagePack: both record types use
/// `#[serde(flatten)]`, which rmp can't round-trip — so this store deliberately
/// diverges from [`crate`]'s sibling `PgSessionStore`. That JSON is then **sealed**
/// with the [`SecretVault`] before it is written, so the `data` bytea holds
/// ciphertext, never plaintext secrets. The schema lives in adapter-pg's
/// migrations (it owns all DDL); the pool and vault are injected by `api`.
#[derive(Clone, Debug)]
pub struct AtprotoAuthStore {
    pool: PgPool,
    vault: SecretVault,
}

impl AtprotoAuthStore {
    /// Wrap a connection `pool` and a [`SecretVault`] (both injected by `api`) as
    /// an OAuth store. The `atproto_oauth` tables it reads/writes are created by
    /// adapter-pg's migrations, which own all DDL; this type never touches the
    /// schema. The `vault` seals every stored blob at rest under the one custody
    /// root key.
    pub fn new(pool: PgPool, vault: SecretVault) -> Self {
        Self { pool, vault }
    }

    /// AEAD associated data binding a session blob to its `(account_did, session_id)`
    /// primary key. The table-name prefix domain-separates it from an auth-request
    /// blob's AAD, and the DID is length-prefixed so the same bytes can't be
    /// re-split into a *different* `(did, session_id)` pair. Not stored — supplied
    /// again on read; see [`SecretVault::seal`].
    fn session_aad(account_did: &str, session_id: &str) -> Vec<u8> {
        let did = account_did.as_bytes();
        let mut aad = Vec::with_capacity(64 + did.len() + session_id.len());
        aad.extend_from_slice(b"atproto_oauth.client_session\0");
        aad.extend_from_slice(&(did.len() as u64).to_le_bytes());
        aad.extend_from_slice(did);
        aad.extend_from_slice(session_id.as_bytes());
        aad
    }

    /// AEAD associated data binding an auth-request blob to its `state` primary
    /// key. `state` is the sole variable field after the table-name prefix, so no
    /// internal length prefix is needed. Not stored — supplied again on read.
    fn auth_request_aad(state: &str) -> Vec<u8> {
        let mut aad = Vec::with_capacity(32 + state.len());
        aad.extend_from_slice(b"atproto_oauth.auth_request\0");
        aad.extend_from_slice(state.as_bytes());
        aad
    }

    /// JSON-encode `value`, then seal it under `aad` so only ciphertext reaches the
    /// column. The transient plaintext JSON (which holds live secrets) is zeroized
    /// as soon as it is sealed. JSON, not MessagePack, for the `#[serde(flatten)]`
    /// reason documented on [`AtprotoAuthStore`].
    fn encode<T: Serialize>(&self, aad: &[u8], value: &T) -> Result<Vec<u8>, SessionStoreError> {
        let plaintext = Zeroizing::new(serde_json::to_vec(value)?);
        self.vault.seal(aad, &plaintext).map_err(seal_error)
    }

    /// Open a sealed blob under `aad` and JSON-decode it — the inverse of
    /// [`encode`](Self::encode). Fails **closed**: a value that is not valid
    /// ciphertext under this vault and `aad` (tampered, wrong key, or a legacy
    /// plaintext row) errors rather than being read as plaintext.
    fn decode<T: DeserializeOwned>(&self, aad: &[u8], data: &[u8]) -> Result<T, SessionStoreError> {
        let plaintext = self.vault.open(aad, data).map_err(seal_error)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }
}

/// Map a sqlx error into jacquard's [`SessionStoreError`] so a database fault
/// surfaces as a store error rather than being misread as "no session".
fn backend(e: sqlx::Error) -> SessionStoreError {
    SessionStoreError::Other(Box::new(e))
}

/// Map a seal/open failure into jacquard's [`SessionStoreError`]. The vault's
/// messages are deliberately opaque (no secret material), and surfacing this as a
/// store error — not a `None` — is what makes an unreadable blob fail closed.
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

    /// Insert-or-replace the session keyed by (DID, session id). The upsert is
    /// what makes jacquard's refresh durable: every rotated token set overwrites
    /// the prior row (`updated_at` bumped), so a later request on any replica
    /// reads the freshest grant (ZMVP-12).
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
    // enumerated — the sign-in flow keys every lookup by DID + session id, and
    // the trait sanctions stores that don't support enumeration.
}
