use std::collections::HashMap;

use jacquard_oauth::session::{ClientSessionData, DpopClientData};
use jacquard_oauth::types::{OAuthTokenType, TokenSet};
use jacquard_oauth::utils::generate_key;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::guarded_http::{GuardedHttp, Script, ScriptedLookup, Timeouts, TxtScript};
use crate::resolver::log_recorder::{lines_at, record_events};

/// Every staged host answers with this public address; the upstream route then
/// delivers the request to the mock.
const PUBLIC: &str = "93.184.216.34";

const HANDLE: &str = "alice.example.com";
const PDS: &str = "pds.example.com";
const AUTH: &str = "auth.example.com";
const ISSUER: &str = "https://auth.example.com";
const REQUEST_URI: &str = "urn:ietf:params:oauth:request_uri:req-1";
const AUTHORIZE: &str = "https://auth.example.com/oauth/authorize";

fn plc(fill: char) -> Did {
    let id: String = std::iter::repeat_n(fill, 24).collect();
    Did::from(format!("did:plc:{id}"))
}

/// The DID the typed handle names.
fn alice() -> Did {
    plc('a')
}

/// Another account on the same PDS.
fn mallory() -> Did {
    plc('m')
}

fn handle(text: &str) -> AtHandle {
    text.parse().expect("a valid handle")
}

fn at_did(did: &Did) -> AtDid {
    AtDid::new_owned(did.as_ref()).expect("a DID")
}

fn resolves_to_public() -> Script {
    Script::Answer(vec![PUBLIC.parse().expect("a test address")])
}

/// A DID document for `did` claiming `claimed`, its PDS on [`PDS`].
fn document(did: &Did, claimed: &str) -> serde_json::Value {
    json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did.as_ref(),
        "alsoKnownAs": [format!("at://{claimed}")],
        "verificationMethod": [],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": format!("https://{PDS}"),
        }],
    })
}

/// The authorization server's metadata, sending browsers to `authorize`.
fn server_metadata(authorize: &str) -> serde_json::Value {
    json!({
        "issuer": ISSUER,
        "authorization_endpoint": authorize,
        "token_endpoint": format!("{ISSUER}/oauth/token"),
        "pushed_authorization_request_endpoint": format!("{ISSUER}/oauth/par"),
        "revocation_endpoint": format!("{ISSUER}/oauth/revoke"),
        "require_pushed_authorization_requests": true,
        "authorization_response_iss_parameter_supported": true,
        "scopes_supported": ["atproto", "transition:generic"],
        "response_types_supported": ["code"],
        "token_endpoint_auth_methods_supported": ["none"],
        "dpop_signing_alg_values_supported": ["ES256"],
    })
}

/// Answer `verb host route` with `response`, ahead of anything mounted before
/// when `first` is set.
async fn mount_with(
    server: &MockServer,
    (verb, host, route): (&str, &str, &str),
    response: ResponseTemplate,
    first: bool,
) {
    let priority = if first { 1 } else { 5 };
    Mock::given(method(verb))
        .and(header("host", host))
        .and(path(route))
        .respond_with(response)
        .with_priority(priority)
        .mount(server)
        .await;
}

async fn mount(
    server: &MockServer,
    verb: &str,
    host: &str,
    route: &str,
    response: ResponseTemplate,
) {
    mount_with(server, (verb, host, route), response, false).await;
}

async fn serve_document(server: &MockServer, did: &Did, body: serde_json::Value) {
    let response = ResponseTemplate::new(200).set_body_json(body);
    mount(server, "GET", "plc.directory", &format!("/{did}"), response).await;
}

/// The PDS, its authorization server and the PAR and revocation endpoints;
/// browsers go to `authorize`.
async fn serve_oauth(server: &MockServer, authorize: &str) {
    let resource = json!({
        "resource": format!("https://{PDS}"),
        "authorization_servers": [ISSUER],
        "scopes_supported": [],
    });
    let resource = ResponseTemplate::new(200).set_body_json(resource);
    mount(
        server,
        "GET",
        PDS,
        "/.well-known/oauth-protected-resource",
        resource,
    )
    .await;
    let metadata = ResponseTemplate::new(200).set_body_json(server_metadata(authorize));
    mount(
        server,
        "GET",
        AUTH,
        "/.well-known/oauth-authorization-server",
        metadata,
    )
    .await;
    let pushed = json!({ "request_uri": REQUEST_URI, "expires_in": 60 });
    let pushed = ResponseTemplate::new(201).set_body_json(pushed);
    mount(server, "POST", AUTH, "/oauth/par", pushed).await;
    mount(
        server,
        "POST",
        AUTH,
        "/oauth/revoke",
        ResponseTemplate::new(204),
    )
    .await;
}

/// The token endpoint, issuing tokens for `sub`.
async fn serve_token(server: &MockServer, sub: &Did) {
    let tokens = json!({
        "access_token": "access",
        "token_type": "DPoP",
        "expires_in": 3600,
        "refresh_token": "refresh",
        "scope": OAUTH_SCOPES,
        "sub": sub.as_ref(),
    });
    let tokens = ResponseTemplate::new(200).set_body_json(tokens);
    mount(server, "POST", AUTH, "/oauth/token", tokens).await;
}

/// How many requests reached `route` on `host`.
async fn hits(server: &MockServer, host: &str, route: &str) -> usize {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| {
            let host_header = request.headers.get("host").and_then(|v| v.to_str().ok());
            host_header == Some(host) && request.url.path() == route
        })
        .count()
}

/// The form fields of the PAR request `server` received.
async fn par_fields(server: &MockServer) -> HashMap<String, String> {
    let requests = server.received_requests().await.unwrap_or_default();
    let par = requests
        .iter()
        .find(|request| request.url.path() == "/oauth/par")
        .expect("a PAR request was sent");
    url::form_urlencoded::parse(&par.body)
        .into_owned()
        .collect()
}

/// A sign-in world: the handle names alice, whose document claims `claimed`;
/// mallory has a document of her own; the PDS and authorization server answer,
/// sending browsers to `authorize`.
struct World {
    server: MockServer,
    authenticator: AtprotoAuthenticator,
    store: AtprotoAuthStore,
    _db: test_support::pg::TestDb,
}

async fn world(authorize: &str, claimed: &str) -> World {
    let server = MockServer::start().await;
    serve_document(&server, &alice(), document(&alice(), claimed)).await;
    serve_document(
        &server,
        &mallory(),
        document(&mallory(), "mallory.example.com"),
    )
    .await;
    serve_oauth(&server, authorize).await;

    let txt = TxtScript::Records(vec![format!("did={}", alice())]);
    let dns = ScriptedLookup::default()
        .with_txt(&format!("_atproto.{HANDLE}."), txt)
        .with("plc.directory.", resolves_to_public())
        .with(&format!("{PDS}."), resolves_to_public())
        .with(&format!("{AUTH}."), resolves_to_public());
    let dns = Arc::new(dns);
    let timeouts = Timeouts {
        fetch: Duration::from_secs(2),
        dns: Duration::from_millis(500),
    };
    let http = GuardedHttp::upstream(*server.address(), dns.clone(), timeouts);
    let resolver = Arc::new(AtprotoIdentityResolver::scripted(http, dns));

    let (pool, db) = test_support::pg::fresh_pool().await;
    let vault = || SecretVault::from_bytes(&[7u8; 32]).expect("32-byte test root key");
    let store = AtprotoAuthStore::new(pool.clone(), vault());
    let redirect_uri =
        Uri::parse("http://127.0.0.1:3000/signin-callback".to_owned()).expect("a redirect URI");
    let authenticator = AtprotoAuthenticator::new(redirect_uri, pool, vault(), resolver);
    World {
        server,
        authenticator,
        store,
        _db: db,
    }
}

/// Start sign-in for [`HANDLE`] and return the `state` the PAR carried.
async fn started(world: &World) -> String {
    world
        .authenticator
        .start(&handle(HANDLE))
        .await
        .expect("sign-in starts");
    par_fields(&world.server).await["state"].clone()
}

/// Complete the callback for `state`, as the PDS would redirect it back.
async fn completed(world: &World, state: &str) -> anyhow::Result<Did> {
    world
        .authenticator
        .complete("code".into(), Some(state.to_owned()), Some(ISSUER.into()))
        .await
}

// ---- start -------------------------------------------------------------

#[tokio::test]
async fn start_binds_the_resolved_did_and_hints_the_typed_handle() {
    let world = world(AUTHORIZE, HANDLE).await;

    let url = world
        .authenticator
        .start(&handle(HANDLE))
        .await
        .expect("sign-in starts");

    let fields = par_fields(&world.server).await;
    assert_eq!(
        fields["login_hint"], HANDLE,
        "the PAR hints the typed handle"
    );
    let stored = world
        .store
        .get_auth_req_info(&fields["state"])
        .await
        .expect("the store answers")
        .expect("the auth request is stored");
    let bound = stored.account_did.map(|did| did.as_str().to_owned());
    let expected_bound = Some(alice().as_ref().to_owned());
    assert_eq!(
        bound, expected_bound,
        "the auth request binds the resolved DID"
    );

    let url = url::Url::parse(&url).expect("an authorization URL");
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(url.as_str().split('?').next(), Some(AUTHORIZE));
    assert_eq!(query["request_uri"], REQUEST_URI);
    assert!(query.contains_key("client_id"));
}

#[tokio::test]
async fn an_authorization_endpoint_failing_the_url_policy_is_refused_before_the_par() {
    let refused_endpoints = [
        "http://auth.example.com/oauth/authorize",
        "https://auth.example.com:8443/oauth/authorize",
        "https://127.0.0.1/oauth/authorize",
        "https://user@auth.example.com/oauth/authorize",
    ];
    for endpoint in refused_endpoints {
        let world = world(endpoint, HANDLE).await;
        let result = world.authenticator.start(&handle(HANDLE)).await;
        assert!(result.is_err(), "{endpoint} must be refused");
        let pars = hits(&world.server, AUTH, "/oauth/par").await;
        assert_eq!(pars, 0, "no PAR is sent toward {endpoint}");
    }
}

#[tokio::test]
async fn a_handle_its_document_does_not_claim_starts_nothing() {
    let world = world(AUTHORIZE, "someone-else.example.com").await;
    let result = world.authenticator.start(&handle(HANDLE)).await;
    assert!(result.is_err(), "an unconfirmed handle cannot sign in");
    let to_pds = hits(&world.server, PDS, "/.well-known/oauth-protected-resource").await;
    let pars = hits(&world.server, AUTH, "/oauth/par").await;
    assert_eq!((to_pds, pars), (0, 0), "nothing past resolution is asked");
}

#[tokio::test]
async fn start_gives_up_at_its_deadline() {
    let world = world(AUTHORIZE, HANDLE).await;
    // A slow PDS: its metadata answers only well after the deadline.
    let slow = ResponseTemplate::new(200).set_delay(Duration::from_secs(5));
    let metadata_route = ("GET", PDS, "/.well-known/oauth-protected-resource");
    mount_with(&world.server, metadata_route, slow, true).await;
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_millis(300), Duration::from_secs(5));

    let begun = std::time::Instant::now();
    let result = authenticator.start(&handle(HANDLE)).await;

    assert!(result.is_err(), "a start past its deadline fails");
    // Well under the 2 s fetch timeout, so only the start deadline explains it.
    assert!(
        begun.elapsed() < Duration::from_secs(1),
        "the deadline cut it off"
    );
}

// ---- complete ----------------------------------------------------------

#[tokio::test]
async fn the_bound_account_completes() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let state = started(&world).await;

    let did = completed(&world, &state)
        .await
        .expect("the bound account signs in");

    assert_eq!(did, alice());
}

#[tokio::test]
async fn another_account_is_revoked_deleted_and_refused() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &mallory()).await;
    let state = started(&world).await;

    let result = completed(&world, &state).await;

    let error = result.expect_err("another account is refused");
    assert!(
        error.is::<AccountMismatch>(),
        "refused as an account mismatch"
    );
    let revocations = hits(&world.server, AUTH, "/oauth/revoke").await;
    assert_eq!(revocations, 1, "the mismatched tokens are revoked");
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the mismatched session row is gone");
}

#[tokio::test]
async fn a_failed_revocation_still_deletes_the_mismatched_session() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &mallory()).await;
    let state = started(&world).await;
    // The authorization server vanishes once the callback is through: its two
    // metadata fetches (the callback's own, the issuer check's) answer, and
    // logout's cannot, so the session must be deleted without a revocation.
    let metadata_route = ("GET", AUTH, "/.well-known/oauth-authorization-server");
    let metadata = ResponseTemplate::new(200).set_body_json(server_metadata(AUTHORIZE));
    Mock::given(method(metadata_route.0))
        .and(header("host", metadata_route.1))
        .and(path(metadata_route.2))
        .respond_with(metadata)
        .with_priority(1)
        .up_to_n_times(2)
        .mount(&world.server)
        .await;
    let gone = ResponseTemplate::new(500);
    mount_with(&world.server, metadata_route, gone, true).await;

    let result = completed(&world, &state).await;

    let error = result.expect_err("another account is refused");
    assert!(error.is::<AccountMismatch>());
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(
        session.is_none(),
        "the session row is gone even without a revocation"
    );
}

#[tokio::test]
async fn an_auth_request_without_a_bound_did_fails_closed() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let state = started(&world).await;
    let mut unbound = world
        .store
        .get_auth_req_info(&state)
        .await
        .expect("the store answers")
        .expect("the auth request is stored");
    unbound.account_did = None;
    world
        .store
        .delete_auth_req_info(&state)
        .await
        .expect("delete");
    world
        .store
        .save_auth_req_info(&unbound)
        .await
        .expect("save");

    let result = completed(&world, &state).await;

    let error = result.expect_err("an unbound request is refused");
    assert!(!error.is::<AccountMismatch>());
    let exchanges = hits(&world.server, AUTH, "/oauth/token").await;
    assert_eq!(exchanges, 0, "no code is exchanged");
    let left = world
        .store
        .get_auth_req_info(&state)
        .await
        .expect("answers");
    assert!(left.is_none(), "the unbound request is deleted");
}

#[tokio::test]
async fn an_unknown_state_exchanges_nothing() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;

    let result = completed(&world, "never-issued").await;

    assert!(result.is_err());
    assert_eq!(hits(&world.server, AUTH, "/oauth/token").await, 0);
}

#[tokio::test]
async fn the_issuer_check_refuses_a_did_web_with_a_path() {
    let world = world(AUTHORIZE, HANDLE).await;
    let with_path = Did::from("did:web:auth.example.com:users:mallory".to_owned());
    serve_token(&world.server, &with_path).await;
    let state = started(&world).await;

    let result = completed(&world, &state).await;

    let error = result.expect_err("the issuer check refuses it");
    assert!(
        !error.is::<AccountMismatch>(),
        "refused before the account check"
    );
    let requests = world.server.received_requests().await.unwrap_or_default();
    let fetched_a_path = requests
        .iter()
        .any(|request| request.url.path().starts_with("/users"));
    assert!(
        !fetched_a_path,
        "no document is fetched from a did:web path"
    );
}

#[tokio::test]
async fn the_issuer_check_refuses_a_mini_doc() {
    let world = world(AUTHORIZE, HANDLE).await;
    let minimal = plc('n');
    let mini_doc = json!({
        "did": minimal.as_ref(),
        "handle": "mini.example.com",
        "signingKey": "did:key:zQ3shunBKsXixLxKtC5qeSG9E4J5RkGN57im31pcTzbNQnm5w",
        "pds": format!("https://{PDS}"),
    });
    serve_document(&world.server, &minimal, mini_doc).await;
    serve_token(&world.server, &minimal).await;
    let state = started(&world).await;

    let result = completed(&world, &state).await;

    let error = result.expect_err("a mini-doc never passes the issuer check");
    assert!(
        !error.is::<AccountMismatch>(),
        "refused before the account check"
    );
    let session = world
        .store
        .get_session(&at_did(&minimal), &state)
        .await
        .expect("answers");
    assert!(session.is_none(), "no session is stored");
}

/// A saved session for `did` under `session_id`, as jacquard's callback stores one.
fn saved_session(did: &Did, session_id: &str) -> ClientSessionData {
    let account_did = at_did(did);
    let dpop_key = generate_key(&[SmolStr::new_static("ES256")]).expect("a DPoP key");
    let token_set = TokenSet {
        iss: SmolStr::new_static(ISSUER),
        sub: account_did.clone(),
        aud: SmolStr::new(format!("https://{PDS}")),
        scope: None,
        refresh_token: Some(SmolStr::new_static("refresh")),
        access_token: SmolStr::new_static("access"),
        token_type: OAuthTokenType::DPoP,
        expires_at: None,
    };
    ClientSessionData {
        account_did,
        session_id: SmolStr::new(session_id),
        host_url: Uri::parse(format!("https://{PDS}"))
            .expect("a PDS URL")
            .to_owned(),
        authserver_url: SmolStr::new_static(ISSUER),
        authserver_token_endpoint: SmolStr::new(format!("{ISSUER}/oauth/token")),
        authserver_revocation_endpoint: None,
        scopes: Scopes::empty(),
        dpop_data: DpopClientData {
            dpop_key,
            dpop_authserver_nonce: SmolStr::default(),
            dpop_host_nonce: SmolStr::default(),
        },
        token_set,
        resolved_scopes: None,
    }
}

/// Serve the authorization server's metadata to the callback's two fetches
/// (its own and the issuer check's), then answer every later fetch with `later`.
async fn metadata_then(server: &MockServer, later: ResponseTemplate) {
    let metadata_route = ("GET", AUTH, "/.well-known/oauth-authorization-server");
    let metadata = ResponseTemplate::new(200).set_body_json(server_metadata(AUTHORIZE));
    Mock::given(method(metadata_route.0))
        .and(header("host", metadata_route.1))
        .and(path(metadata_route.2))
        .respond_with(metadata)
        .with_priority(1)
        .up_to_n_times(2)
        .mount(server)
        .await;
    mount_with(server, metadata_route, later, true).await;
}

#[tokio::test]
async fn a_stalled_revocation_still_refuses_and_leaves_no_session() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &mallory()).await;
    let state = started(&world).await;
    // The authorization server answers the callback, then stalls logout's
    // metadata fetch: the stall must neither strand the row nor change the answer.
    let stalled = ResponseTemplate::new(200)
        .set_body_json(server_metadata(AUTHORIZE))
        .set_delay(Duration::from_secs(3));
    metadata_then(&world.server, stalled).await;
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_secs(1), Duration::from_millis(300));

    let begun = std::time::Instant::now();
    let result = authenticator
        .complete("code".into(), Some(state.clone()), Some(ISSUER.into()))
        .await;

    let error = result.expect_err("another account is refused");
    assert!(error.is::<AccountMismatch>(), "still an account mismatch");
    assert!(
        begun.elapsed() < Duration::from_secs(2),
        "the revocation gave up"
    );
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the row is gone before any revocation");
}

#[tokio::test]
async fn the_complete_deadline_cuts_off_the_callback_and_discards_its_session() {
    let world = world(AUTHORIZE, HANDLE).await;
    let state = started(&world).await;
    // A token exchange that answers only after the deadline, and a session
    // already saved under this sign-in's state, as if the callback had got that far.
    let slow = ResponseTemplate::new(200).set_delay(Duration::from_secs(5));
    mount(&world.server, "POST", AUTH, "/oauth/token", slow).await;
    let saved = saved_session(&mallory(), &state);
    world.store.upsert_session(saved).await.expect("saved");
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_millis(300), Duration::from_secs(5));

    let begun = std::time::Instant::now();
    let result = authenticator
        .complete("code".into(), Some(state.clone()), Some(ISSUER.into()))
        .await;

    let error = result.expect_err("a callback past its deadline fails");
    assert!(!error.is::<AccountMismatch>());
    assert!(
        begun.elapsed() < Duration::from_secs(1),
        "the deadline cut it off"
    );
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(
        session.is_none(),
        "every session under the state is deleted"
    );
}

#[tokio::test]
async fn a_dropped_callback_request_still_cleans_up() {
    let world = world(AUTHORIZE, HANDLE).await;
    let state = started(&world).await;
    // The token exchange for another account answers after the request is gone.
    let tokens = json!({
        "access_token": "access",
        "token_type": "DPoP",
        "expires_in": 3600,
        "scope": OAUTH_SCOPES,
        "sub": mallory().as_ref(),
    });
    let slow_tokens = ResponseTemplate::new(200)
        .set_body_json(tokens)
        .set_delay(Duration::from_millis(500));
    mount(&world.server, "POST", AUTH, "/oauth/token", slow_tokens).await;

    let callback =
        world
            .authenticator
            .complete("code".into(), Some(state.clone()), Some(ISSUER.into()));
    let dropped = tokio::time::timeout(Duration::from_millis(100), callback).await;
    assert!(dropped.is_err(), "the request went away mid-callback");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let revocations = hits(&world.server, AUTH, "/oauth/revoke").await;
    assert_eq!(revocations, 1, "the callback ran on and revoked");
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "and deleted the session");
}

#[tokio::test]
async fn sign_in_logs_only_the_failure_class_at_info() {
    let unconfirmed = world(AUTHORIZE, "someone-else.example.com").await;
    let refused_endpoint = world("http://auth.example.com/oauth/authorize", HANDLE).await;
    let mismatched = world(AUTHORIZE, HANDLE).await;
    serve_token(&mismatched.server, &mallory()).await;
    let state = started(&mismatched).await;

    let ((), events) = record_events(async {
        let typed = handle(HANDLE);
        let _ = unconfirmed.authenticator.start(&typed).await;
        let _ = refused_endpoint.authenticator.start(&typed).await;
        let _ = completed(&mismatched, &state).await;
    })
    .await;

    let info = lines_at(&events, tracing::Level::INFO, false);
    let expected_info = vec![
        "message=sign-in failed failure=handle resolution failed: NotConfirmed ".to_string(),
        "message=sign-in failed failure=authorization endpoint refused ".to_string(),
        "message=sign-in failed failure=signed in as a different account ".to_string(),
    ];
    assert_eq!(info, expected_info);
    for line in lines_at(&events, tracing::Level::INFO, true) {
        for secret in [
            HANDLE,
            PDS,
            AUTH,
            "plc.directory",
            "did:plc",
            state.as_str(),
        ] {
            assert!(
                !line.contains(secret),
                "an info line carries {secret:?}: {line}"
            );
        }
    }
}

#[tokio::test]
async fn a_failure_cause_reaches_the_debug_line_only() {
    let cause = anyhow::anyhow!("TLS handshake with pds.example.com failed");
    let failure = SigninFailure::Resolve(ResolveError::Refused(cause));

    let ((), events) = record_events(async { log_failure(&failure) }).await;

    let info = lines_at(&events, tracing::Level::INFO, false);
    let debug = lines_at(&events, tracing::Level::DEBUG, false);
    assert!(
        info.iter().all(|line| !line.contains("pds.example.com")),
        "the cause stays off info: {info:?}"
    );
    assert!(
        debug.iter().any(|line| line.contains("pds.example.com")),
        "the cause behind a ResolveError reaches debug: {debug:?}"
    );
}

#[tokio::test]
async fn a_replayed_callback_is_refused_and_leaves_the_live_session() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let state = started(&world).await;
    completed(&world, &state)
        .await
        .expect("the first callback signs in");
    let live = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(live.is_some(), "the sign-in stored its session");

    // A reload, the back button, or anyone who saw the callback URL.
    let replay = completed(&world, &state).await;

    assert!(replay.is_err(), "a state is single-use");
    let still_live = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(still_live.is_some(), "the replay deleted nothing");
}
