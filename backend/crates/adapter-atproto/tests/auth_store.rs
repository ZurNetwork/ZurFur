//! Round-trips `AtprotoAuthStore` against a throwaway PostgreSQL container,
//! proving the migration-created `atproto_oauth.*` tables persist OAuth state —
//! and, critically, that the `#[serde(flatten)]`-heavy `ClientSessionData` and
//! `AuthRequestData` survive the JSON encode/decode this store relies on.
//! Requires a container runtime socket (DOCKER_HOST honored).
use adapter_atproto::AtprotoAuthStore;
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

/// A store on a fresh, fully migrated private database (a clone of the shared
/// template, which the real migrations gave the `atproto_oauth` schema — see
/// `test_support::pg`). The second element keeps the shared container alive
/// for the test's duration.
async fn fresh_store() -> (AtprotoAuthStore, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    (AtprotoAuthStore::new(pool), db)
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
