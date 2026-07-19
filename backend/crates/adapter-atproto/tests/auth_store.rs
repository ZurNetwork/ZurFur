//! Round-trips `AtprotoAuthStore` against a throwaway PostgreSQL container,
//! proving the migration-created `atproto_oauth.*` tables persist OAuth state —
//! and, critically, that the `#[serde(flatten)]`-heavy `ClientSessionData` and
//! `AuthRequestData` survive the JSON encode/decode this store relies on.
//! Requires a container runtime socket (DOCKER_HOST honored).
use adapter_atproto::{AtprotoAuthStore, SecretVault};
use fluent_uri::Uri;
use jacquard_common::types::did::Did;
use jacquard_oauth::{
    authstore::ClientAuthStore,
    scopes::Scopes,
    session::{AuthRequestData, ClientSessionData, DpopClientData, DpopReqData},
    types::{OAuthTokenType, TokenSet},
    utils::generate_key,
};
use smol_str::SmolStr;
use sqlx::PgPool;

/// A fixed 32-byte root key for the test vault. Real deployments source this from
/// `ZURFUR_DID_KEY_ROOT_KEY` (the same key the custody store uses); tests only
/// need *a* valid key so seal/open round-trips.
fn test_vault() -> SecretVault {
    SecretVault::from_bytes(&[7u8; 32]).expect("32-byte test root key")
}

/// A store on a fresh, fully migrated private database (a clone of the shared
/// template, which the real migrations gave the `atproto_oauth` schema — see
/// `test_support::pg`). The second element keeps the shared container alive
/// for the test's duration.
async fn fresh_store() -> (AtprotoAuthStore, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    (AtprotoAuthStore::new(pool, test_vault()), db)
}

/// Like [`fresh_store`], but also hands back the pool so a test can read the raw
/// at-rest `data` bytea directly — bypassing the store's decrypt path — to prove
/// what actually landed on disk.
async fn fresh_store_and_pool() -> (AtprotoAuthStore, PgPool, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    (AtprotoAuthStore::new(pool.clone(), test_vault()), pool, db)
}

fn client_session(did: &'static str, session_id: &'static str) -> ClientSessionData {
    let account_did = Did::new_static(did).expect("valid did");
    ClientSessionData {
        account_did: account_did.clone(),
        session_id: SmolStr::new_static(session_id),
        host_url: Uri::parse("https://pds.example.com")
            .expect("valid uri")
            .to_owned(),
        authserver_url: SmolStr::new_static("https://issuer.example.com"),
        authserver_token_endpoint: SmolStr::new_static("https://issuer.example.com/token"),
        authserver_revocation_endpoint: None,
        scopes: Scopes::empty(),
        dpop_data: DpopClientData {
            dpop_key: generate_key(&[SmolStr::new_static("ES256")]).expect("dpop key"),
            dpop_authserver_nonce: SmolStr::default(),
            dpop_host_nonce: SmolStr::default(),
        },
        token_set: TokenSet {
            iss: SmolStr::new_static("https://issuer.example.com"),
            sub: account_did,
            aud: SmolStr::new_static("https://pds.example.com"),
            scope: None,
            refresh_token: Some(SmolStr::new_static("refresh-token")),
            access_token: SmolStr::new_static("access-token"),
            token_type: OAuthTokenType::DPoP,
            expires_at: None,
        },
        resolved_scopes: None,
    }
}

fn auth_request(state: &'static str) -> AuthRequestData {
    AuthRequestData {
        state: SmolStr::new_static(state),
        authserver_url: SmolStr::new_static("https://issuer.example.com"),
        account_did: Some(Did::new_static("did:plc:alice").expect("valid did")),
        scopes: Scopes::empty(),
        request_uri: SmolStr::new_static("urn:ietf:params:oauth:request_uri:abc"),
        authserver_token_endpoint: SmolStr::new_static("https://issuer.example.com/token"),
        authserver_revocation_endpoint: None,
        pkce_verifier: SmolStr::new_static("pkce-verifier"),
        dpop_data: DpopReqData {
            dpop_key: generate_key(&[SmolStr::new_static("ES256")]).expect("dpop key"),
            dpop_authserver_nonce: None,
        },
    }
}

#[tokio::test]
async fn client_session_upsert_get_delete_roundtrip() {
    let (store, _container) = fresh_store().await;
    let did: Did = Did::new_static("did:plc:alice").expect("valid did");

    let session = client_session("did:plc:alice", "session-1");
    store.upsert_session(session.clone()).await.expect("upsert");

    // The full record — token set + DPoP private key — round-trips byte-for-byte.
    let loaded = store
        .get_session(&did, "session-1")
        .await
        .expect("get")
        .expect("session present after upsert");
    assert_eq!(loaded, session, "stored session must round-trip exactly");

    // upsert replaces in place: a rotated access token (what refresh writes).
    let mut rotated = session.clone();
    rotated.token_set.access_token = SmolStr::new_static("access-token-2");
    store.upsert_session(rotated.clone()).await.expect("upsert");
    let reloaded = store
        .get_session(&did, "session-1")
        .await
        .expect("get")
        .expect("session present after rotate");
    assert_eq!(reloaded.token_set.access_token.as_str(), "access-token-2");

    // delete_session is jacquard's "the grant ended honestly" path.
    store
        .delete_session(&did, "session-1")
        .await
        .expect("delete");
    assert!(
        store
            .get_session(&did, "session-1")
            .await
            .expect("get")
            .is_none(),
        "session must be gone after delete"
    );
}

#[tokio::test]
async fn auth_request_save_get_delete_roundtrip() {
    let (store, _container) = fresh_store().await;

    let req = auth_request("state-xyz");
    store.save_auth_req_info(&req).await.expect("save");

    let loaded = store
        .get_auth_req_info("state-xyz")
        .await
        .expect("get")
        .expect("auth request present after save");
    assert_eq!(loaded, req, "stored auth request must round-trip exactly");

    // The callback deletes the in-flight request once consumed.
    store
        .delete_auth_req_info("state-xyz")
        .await
        .expect("delete");
    assert!(
        store
            .get_auth_req_info("state-xyz")
            .await
            .expect("get")
            .is_none(),
        "auth request must be gone after delete"
    );
}

#[tokio::test]
async fn missing_session_returns_none() {
    let (store, _container) = fresh_store().await;
    let did: Did = Did::new_static("did:plc:nobody").expect("valid did");
    assert!(
        store
            .get_session(&did, "absent")
            .await
            .expect("get")
            .is_none(),
        "absent session must read as None, not error"
    );
}

/// The whole point of the fix: what lands in `client_session.data` is ciphertext,
/// not the plaintext JSON. The DPoP private key + tokens must not be recoverable
/// from a raw column read (the leaked-backup / read-replica threat).
#[tokio::test]
async fn stored_session_bytes_are_not_plaintext_json() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    store.upsert_session(session.clone()).await.expect("upsert");

    // Read the at-rest bytes straight from the column, bypassing the store.
    let raw: Vec<u8> = sqlx::query_scalar(
        "SELECT data FROM atproto_oauth.client_session WHERE account_did = $1 AND session_id = $2",
    )
    .bind("did:plc:alice")
    .bind("session-1")
    .fetch_one(&pool)
    .await
    .expect("row present after upsert");

    // Not the plaintext JSON, and no secret/identifier survives in the clear.
    let plaintext_json = serde_json::to_vec(&session).expect("json");
    assert_ne!(
        raw, plaintext_json,
        "at-rest bytes must not be the plaintext JSON"
    );
    for needle in [
        b"refresh-token".as_slice(),
        b"access-token".as_slice(),
        b"did:plc:alice".as_slice(),
    ] {
        assert!(
            !raw.windows(needle.len()).any(|w| w == needle),
            "secret/identifier {:?} leaked into the at-rest bytes",
            std::str::from_utf8(needle).unwrap()
        );
    }
    assert!(
        serde_json::from_slice::<ClientSessionData>(&raw).is_err(),
        "the ciphertext must not deserialize as a session"
    );
}

/// The in-flight `auth_request.data` is sealed too — the PKCE verifier + DPoP key
/// must not sit in the clear.
#[tokio::test]
async fn stored_auth_request_bytes_are_not_plaintext_json() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let req = auth_request("state-xyz");
    store.save_auth_req_info(&req).await.expect("save");

    let raw: Vec<u8> =
        sqlx::query_scalar("SELECT data FROM atproto_oauth.auth_request WHERE state = $1")
            .bind("state-xyz")
            .fetch_one(&pool)
            .await
            .expect("row present after save");

    for needle in [b"pkce-verifier".as_slice(), b"did:plc:alice".as_slice()] {
        assert!(
            !raw.windows(needle.len()).any(|w| w == needle),
            "secret/identifier {:?} leaked into the at-rest bytes",
            std::str::from_utf8(needle).unwrap()
        );
    }
    assert!(
        serde_json::from_slice::<AuthRequestData>(&raw).is_err(),
        "the ciphertext must not deserialize as an auth request"
    );
}

/// A value that is not valid ciphertext under the store's vault (here, a legacy
/// plaintext-JSON row written straight into the column) must fail CLOSED on read —
/// an error, never a silent downgrade that hands back the plaintext.
#[tokio::test]
async fn non_sealed_row_fails_closed_on_read() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    let plaintext_json = serde_json::to_vec(&session).expect("json");

    sqlx::query(
        "INSERT INTO atproto_oauth.client_session (account_did, session_id, data) \
         VALUES ($1, $2, $3)",
    )
    .bind("did:plc:alice")
    .bind("session-1")
    .bind(&plaintext_json)
    .execute(&pool)
    .await
    .expect("insert plaintext row");

    let did: Did = Did::new_static("did:plc:alice").expect("valid did");
    assert!(
        store.get_session(&did, "session-1").await.is_err(),
        "a non-sealed value must fail closed, not pass through as plaintext"
    );
}

/// The AAD is wired to the actual row key: a sealed blob grafted onto a different
/// `(account_did, session_id)` row fails the tag check on read, so an attacker
/// with DB write access cannot move one session's secret onto another row.
#[tokio::test]
async fn sealed_session_is_bound_to_its_row_key() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    store.upsert_session(session).await.expect("upsert");

    // Lift the sealed blob and graft it onto a second session_id for the same DID.
    let raw: Vec<u8> = sqlx::query_scalar(
        "SELECT data FROM atproto_oauth.client_session WHERE account_did = $1 AND session_id = $2",
    )
    .bind("did:plc:alice")
    .bind("session-1")
    .fetch_one(&pool)
    .await
    .expect("row present");
    sqlx::query(
        "INSERT INTO atproto_oauth.client_session (account_did, session_id, data) \
         VALUES ($1, $2, $3)",
    )
    .bind("did:plc:alice")
    .bind("session-2")
    .bind(&raw)
    .execute(&pool)
    .await
    .expect("graft blob onto session-2");

    let did: Did = Did::new_static("did:plc:alice").expect("valid did");
    assert!(
        store.get_session(&did, "session-2").await.is_err(),
        "a blob grafted onto another row key must fail the AAD check"
    );
    assert!(
        store
            .get_session(&did, "session-1")
            .await
            .expect("get")
            .is_some(),
        "the legitimate row still opens"
    );
}
