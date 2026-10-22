//! Round-trips `AtprotoAuthStore` against a throwaway PostgreSQL container,
//! proving the migration-created `atproto_oauth.*` tables persist OAuth state —
//! and, critically, that the `#[serde(flatten)]`-heavy `ClientSessionData` and
//! `AuthRequestData` survive the JSON encode/decode this store relies on — and
//! that every stored blob is sealed under the sign-in-secrets key, bound to its row.
//! Requires a container runtime socket (DOCKER_HOST honored).
use adapter_atproto::AtprotoAuthStore;
use domain::elements::custody::{CustodyError, CustodySubject};
use fluent_uri::Uri;
use jacquard_common::{session::SessionStoreError, types::did::Did};
use jacquard_oauth::{
    authstore::ClientAuthStore,
    scopes::Scopes,
    session::{AuthRequestData, ClientSessionData, DpopClientData, DpopReqData},
    types::{OAuthTokenType, TokenSet},
    utils::generate_key,
};
use smol_str::SmolStr;
use sqlx::PgPool;
use test_support::custody::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_BLOB_STARTING_0X01_HEX, hex_bytes, test_custody,
};

/// The version byte every stored envelope starts with.
const ENVELOPE_VERSION: u8 = 0x01;

/// How every sign-in-secrets key id starts.
const SIGNIN_KEY_ID_PREFIX: &str = "hkdf:signin_secrets:";

/// A store on a fresh, fully migrated private database (a clone of the shared
/// template, which the real migrations gave the `atproto_oauth` schema — see
/// `test_support::pg`), sealing with the test custodian's sign-in handle. The
/// second element keeps the shared container alive for the test's duration.
async fn fresh_store() -> (AtprotoAuthStore, impl Sized) {
    let (store, _pool, db) = fresh_store_and_pool().await;
    (store, db)
}

/// Like [`fresh_store`], but also hands back the pool so a test can read the raw
/// at-rest `data` bytea directly — bypassing the store's decrypt path — to prove
/// what actually landed on disk.
async fn fresh_store_and_pool() -> (AtprotoAuthStore, PgPool, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let sealer = test_custody().signin_secrets;
    (AtprotoAuthStore::new(pool.clone(), sealer), pool, db)
}

/// The key id a stored envelope's header names: the version byte, the key id's
/// length byte, then the key id.
fn stored_key_id(data: &[u8]) -> String {
    let [_version, key_id_length, rest @ ..] = data else {
        panic!("a stored envelope has a header");
    };
    let key_id = &rest[..usize::from(*key_id_length)];
    String::from_utf8(key_id.to_vec()).expect("a key id is text")
}

/// The custodian's refusal behind a failed read: the blob didn't open, rather
/// than opening and failing to decode. `None` for any other failure.
fn custody_refusal(error: &SessionStoreError) -> Option<&CustodyError> {
    let SessionStoreError::Other(source) = error else {
        return None;
    };
    source.downcast_ref::<CustodyError>()
}

/// The raw `data` of the session stored under `did` and `session_id`.
async fn raw_session(pool: &PgPool, did: &str, session_id: &str) -> Vec<u8> {
    sqlx::query_scalar(
        "SELECT data FROM atproto_oauth.client_session WHERE account_did = $1 AND session_id = $2",
    )
    .bind(did)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .expect("the session row is present")
}

/// The raw `data` of the auth request stored under `state`.
async fn raw_auth_request(pool: &PgPool, state: &str) -> Vec<u8> {
    sqlx::query_scalar("SELECT data FROM atproto_oauth.auth_request WHERE state = $1")
        .bind(state)
        .fetch_one(pool)
        .await
        .expect("the auth request row is present")
}

/// Write `data` straight into a session row, bypassing the store.
async fn plant_session(pool: &PgPool, did: &str, session_id: &str, data: &[u8]) {
    sqlx::query(
        "INSERT INTO atproto_oauth.client_session (account_did, session_id, data) \
         VALUES ($1, $2, $3)",
    )
    .bind(did)
    .bind(session_id)
    .bind(data)
    .execute(pool)
    .await
    .expect("insert the planted session row");
}

/// Write `data` straight into an auth request row, bypassing the store.
async fn plant_auth_request(pool: &PgPool, state: &str, data: &[u8]) {
    sqlx::query("INSERT INTO atproto_oauth.auth_request (state, data) VALUES ($1, $2)")
        .bind(state)
        .bind(data)
        .execute(pool)
        .await
        .expect("insert the planted auth request row");
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
            scope: Some(SmolStr::new_static("atproto")),
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
async fn a_session_granting_anything_but_exactly_atproto_is_never_written() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let refused_grants = [
        Some("atproto transition:generic"),
        Some("transition:generic"),
        Some(""),
        None,
    ];

    for granted in refused_grants {
        let mut session = client_session("did:plc:alice", "session-1");
        session.token_set.scope = granted.map(SmolStr::new);
        let saved = store.upsert_session(session).await;
        assert!(saved.is_err(), "a grant of {granted:?} is refused");
    }

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM atproto_oauth.client_session")
        .fetch_one(&pool)
        .await
        .expect("the session table answers");
    assert_eq!(rows, 0, "no refused session reached the table");
    let identity_only = client_session("did:plc:alice", "session-1");
    store
        .upsert_session(identity_only)
        .await
        .expect("exactly atproto is saved");
}

#[tokio::test]
async fn a_held_refused_session_never_shows_in_the_stores_debug() {
    let (store, _container) = fresh_store().await;
    let mut wider = client_session("did:plc:alice", "session-1");
    wider.token_set.scope = Some(SmolStr::new_static("atproto transition:generic"));
    let saved = store.upsert_session(wider).await;
    assert!(saved.is_err(), "the wider grant is refused, and held");

    let shown = format!("{store:?}");

    assert!(shown.contains("held: 1"), "the held count shows: {shown}");
    for secret in [
        "refresh-token",
        "access-token",
        "session-1",
        "did:plc:alice",
    ] {
        assert!(
            !shown.contains(secret),
            "the Debug shows {secret:?}: {shown}"
        );
    }
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

/// Every blob the store writes, in either table, is a version-1 envelope under a
/// sign-in-secrets key: the purpose is in the stored bytes.
#[tokio::test]
async fn every_stored_blob_is_sealed_under_the_sign_in_key() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    store.upsert_session(session).await.expect("upsert");
    let request = auth_request("state-xyz");
    store.save_auth_req_info(&request).await.expect("save");

    let session_data = raw_session(&pool, "did:plc:alice", "session-1").await;
    let request_data = raw_auth_request(&pool, "state-xyz").await;

    for data in [session_data, request_data] {
        assert_eq!(
            data.first(),
            Some(&ENVELOPE_VERSION),
            "a version-1 envelope"
        );
        let key_id = stored_key_id(&data);
        assert!(
            key_id.starts_with(SIGNIN_KEY_ID_PREFIX),
            "sealed under a sign-in-secrets key, not {key_id:?}"
        );
    }
}

/// A 0.1 blob (`24-byte nonce ‖ ciphertext`) left in either table fails closed on
/// read: an error naming no row key, never plaintext, never "no session". One of
/// the fixtures starts with the version byte.
#[tokio::test]
async fn a_legacy_row_fails_closed() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let did: Did = Did::new_static("did:plc:alice").expect("valid did");
    let legacy_blobs = [
        ("session-1", "state-1", hex_bytes(LEGACY_ALICE_BLOB_HEX)),
        (
            "session-2",
            "state-2",
            hex_bytes(LEGACY_ALICE_BLOB_STARTING_0X01_HEX),
        ),
    ];

    for (session_id, state, legacy_blob) in legacy_blobs {
        plant_session(&pool, "did:plc:alice", session_id, &legacy_blob).await;
        plant_auth_request(&pool, state, &legacy_blob).await;

        let session_error = store
            .get_session(&did, session_id)
            .await
            .expect_err("a legacy session row fails closed");
        let request_error = store
            .get_auth_req_info(state)
            .await
            .expect_err("a legacy auth request row fails closed");

        for error in [&session_error, &request_error] {
            assert!(
                custody_refusal(error).is_some(),
                "the legacy blob is refused by the custodian: {error}"
            );
        }
        for error in [session_error.to_string(), request_error.to_string()] {
            for hidden in ["did:plc:alice", session_id, state, "hkdf"] {
                assert!(
                    !error.contains(hidden),
                    "the error names {hidden:?}: {error}"
                );
            }
        }
    }
}

/// A session's sealed blob copied into an auth request row whose `state` is the
/// session's id (jacquard keys a sign-in's session by its `state`) doesn't open:
/// the table is part of the subject.
#[tokio::test]
async fn a_session_blob_wont_open_as_an_auth_request() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "state-xyz");
    store.upsert_session(session).await.expect("upsert");
    let session_data = raw_session(&pool, "did:plc:alice", "state-xyz").await;

    plant_auth_request(&pool, "state-xyz", &session_data).await;
    let read_error = store
        .get_auth_req_info("state-xyz")
        .await
        .expect_err("a session blob is no auth request");

    let refusal = custody_refusal(&read_error);
    assert_eq!(
        refusal,
        Some(&CustodyError::Unauthentic),
        "a session blob doesn't open as an auth request: {read_error}"
    );
}

/// An envelope sealed under the DID-custody key, to the very subject a session
/// row's blob is sealed to, doesn't open through the store: its handle opens
/// sign-in secrets only.
#[tokio::test]
async fn the_sign_in_handle_cannot_open_did_custody() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    let plaintext = serde_json::to_vec(&session).expect("json");
    let parts: [&[u8]; 3] = [b"client_session", b"did:plc:alice", b"session-1"];
    let subject = CustodySubject::try_from(parts.as_slice()).expect("the parts encode");
    let did_custody_envelope = test_custody()
        .did_custody
        .seal(&subject, &plaintext)
        .await
        .expect("the did-custody key seals");

    plant_session(
        &pool,
        "did:plc:alice",
        "session-1",
        did_custody_envelope.as_ref(),
    )
    .await;
    let did: Did = Did::new_static("did:plc:alice").expect("valid did");
    let read_error = store
        .get_session(&did, "session-1")
        .await
        .expect_err("a did-custody envelope is no session");

    assert!(
        custody_refusal(&read_error).is_some(),
        "a did-custody envelope doesn't open as a session: {read_error}"
    );
}

/// The store's `Debug` shows the sealer's purpose, never a key or a key id.
#[tokio::test]
async fn the_stores_debug_shows_no_key() {
    let (store, pool, _container) = fresh_store_and_pool().await;
    let session = client_session("did:plc:alice", "session-1");
    store.upsert_session(session).await.expect("upsert");
    let key_id = stored_key_id(&raw_session(&pool, "did:plc:alice", "session-1").await);

    let shown = format!("{store:?}");

    assert!(
        shown.contains("ScopedSealer { purpose: SigninSecrets }"),
        "the sealer shows its purpose: {shown}"
    );
    for hidden in [key_id.as_str(), "hkdf", "94, 94", "5e5e"] {
        assert!(
            !shown.contains(hidden),
            "the Debug shows {hidden:?}: {shown}"
        );
    }
}
