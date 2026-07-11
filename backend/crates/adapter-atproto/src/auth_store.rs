//! Postgres persistence for jacquard's OAuth state — see [`AtprotoAuthStore`].
//!
//! This module implements jacquard's [`ClientAuthStore`] trait against the
//! `atproto_oauth` Postgres schema so the OAuth grant for a visitor outlives a
//! single process. It holds no protocol logic: jacquard owns refresh, expiry
//! skew, and the single-flight lock; this is just durable storage (ZMVP-12).

use crate::queries::auth_store as sql;
use jacquard_common::{bos::BosStr, session::SessionStoreError, types::did::Did};
use jacquard_oauth::{
    authstore::ClientAuthStore,
    session::{AuthRequestData, ClientSessionData},
};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::PgPool;

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
/// diverges from [`crate`]'s sibling `PgSessionStore`. The schema lives in
/// adapter-pg's migrations (it owns all DDL); the pool is injected by `api`.
#[derive(Clone, Debug)]
pub struct AtprotoAuthStore {
    pool: PgPool,
}

impl AtprotoAuthStore {
    /// Wrap a connection `pool` (injected by `api`) as an OAuth store. The
    /// `atproto_oauth` tables it reads/writes are created by adapter-pg's
    /// migrations, which own all DDL; this type never touches the schema.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// JSON-encode a stored value. JSON, not MessagePack: both jacquard record types
/// use `#[serde(flatten)]`, which rmp can't round-trip (see [`AtprotoAuthStore`]).
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SessionStoreError> {
    Ok(serde_json::to_vec(value)?)
}

/// Decode a stored JSON value, the inverse of [`encode`].
fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, SessionStoreError> {
    Ok(serde_json::from_slice(data)?)
}

/// Map a sqlx error into jacquard's [`SessionStoreError`] so a database fault
/// surfaces as a store error rather than being misread as "no session".
fn backend(e: sqlx::Error) -> SessionStoreError {
    SessionStoreError::Other(Box::new(e))
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
        data.map(|data| decode(&data)).transpose()
    }

    /// Insert-or-replace the session keyed by (DID, session id). The upsert is
    /// what makes jacquard's refresh durable: every rotated token set overwrites
    /// the prior row (`updated_at` bumped), so a later request on any replica
    /// reads the freshest grant (ZMVP-12).
    async fn upsert_session(&self, session: ClientSessionData) -> Result<(), SessionStoreError> {
        let data = encode(&session)?;
        sql::upsert_session(
            &self.pool,
            session.account_did.as_ref(),
            AsRef::<str>::as_ref(&session.session_id),
            &data,
        )
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
        data.map(|data| decode(&data)).transpose()
    }

    async fn save_auth_req_info(
        &self,
        auth_req_info: &AuthRequestData,
    ) -> Result<(), SessionStoreError> {
        let data = encode(auth_req_info)?;
        sql::save_auth_req_info(
            &self.pool,
            AsRef::<str>::as_ref(&auth_req_info.state),
            &data,
        )
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
