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
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SessionStoreError> {
    Ok(serde_json::to_vec(value)?)
}

fn decode<T: DeserializeOwned>(data: &[u8]) -> Result<T, SessionStoreError> {
    Ok(serde_json::from_slice(data)?)
}

fn backend(e: sqlx::Error) -> SessionStoreError {
    SessionStoreError::Other(Box::new(e))
}

impl ClientAuthStore for AtprotoAuthStore {
    async fn get_session<D: BosStr + Send + Sync>(
        &self,
        did: &Did<D>,
        session_id: &str,
    ) -> Result<Option<ClientSessionData>, SessionStoreError> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT data FROM atproto_oauth.client_session
             WHERE account_did = $1 AND session_id = $2",
        )
        .bind(did.as_ref())
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;
        row.map(|(data,)| decode(&data)).transpose()
    }

    async fn upsert_session(&self, session: ClientSessionData) -> Result<(), SessionStoreError> {
        let data = encode(&session)?;
        sqlx::query(
            "INSERT INTO atproto_oauth.client_session (account_did, session_id, data, updated_at)
             VALUES ($1, $2, $3, now())
             ON CONFLICT (account_did, session_id) DO UPDATE
             SET data = excluded.data, updated_at = now()",
        )
        .bind(session.account_did.as_ref())
        .bind(AsRef::<str>::as_ref(&session.session_id))
        .bind(data)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn delete_session<D: BosStr + Send + Sync>(
        &self,
        did: &Did<D>,
        session_id: &str,
    ) -> Result<(), SessionStoreError> {
        sqlx::query(
            "DELETE FROM atproto_oauth.client_session
             WHERE account_did = $1 AND session_id = $2",
        )
        .bind(did.as_ref())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn get_auth_req_info(
        &self,
        state: &str,
    ) -> Result<Option<AuthRequestData>, SessionStoreError> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT data FROM atproto_oauth.auth_request WHERE state = $1")
                .bind(state)
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        row.map(|(data,)| decode(&data)).transpose()
    }

    async fn save_auth_req_info(
        &self,
        auth_req_info: &AuthRequestData,
    ) -> Result<(), SessionStoreError> {
        let data = encode(auth_req_info)?;
        sqlx::query(
            "INSERT INTO atproto_oauth.auth_request (state, data, created_at)
             VALUES ($1, $2, now())
             ON CONFLICT (state) DO UPDATE SET data = excluded.data",
        )
        .bind(AsRef::<str>::as_ref(&auth_req_info.state))
        .bind(data)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn delete_auth_req_info(&self, state: &str) -> Result<(), SessionStoreError> {
        sqlx::query("DELETE FROM atproto_oauth.auth_request WHERE state = $1")
            .bind(state)
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }

    // `list_session_keys` keeps the trait default (empty): this store is not
    // enumerated — the sign-in flow keys every lookup by DID + session id, and
    // the trait sanctions stores that don't support enumeration.
}
