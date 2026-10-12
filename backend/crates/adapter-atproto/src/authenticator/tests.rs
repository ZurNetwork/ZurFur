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
/// The tokens the mock token endpoint issues.
const ACCESS_TOKEN: &str = "issued-access-token";
const REFRESH_TOKEN: &str = "issued-refresh-token";

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

/// The token endpoint, issuing tokens for `sub` with the scope asked for.
async fn serve_token(server: &MockServer, sub: &Did) {
    serve_token_granting(server, sub, Some(OAUTH_SCOPES)).await;
}

/// The token endpoint, issuing tokens for `sub` that grant `scope`, or name
/// no scope at all.
async fn serve_token_granting(server: &MockServer, sub: &Did, scope: Option<&str>) {
    let mut tokens = json!({
        "access_token": ACCESS_TOKEN,
        "token_type": "DPoP",
        "expires_in": 3600,
        "refresh_token": REFRESH_TOKEN,
        "sub": sub.as_ref(),
    });
    if let Some(scope) = scope {
        tokens["scope"] = json!(scope);
    }
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

/// The `token` field of every revocation request `server` received, in the
/// order they arrived.
async fn revoked_tokens(server: &MockServer) -> Vec<String> {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| request.url.path() == "/oauth/revoke")
        .filter_map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(name, _)| name == "token")
                .map(|(_, token)| token.into_owned())
        })
        .collect()
}

/// [`revoked_tokens`], once `count` revocations have arrived or 10 s have
/// passed, whichever is first: a background revocation lands after the
/// callback returns.
async fn revocations_arrived(server: &MockServer, count: usize) -> Vec<String> {
    let arriving = async {
        while revoked_tokens(server).await.len() < count {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(10), arriving).await;
    revoked_tokens(server).await
}

/// How many OAuth sessions and auth requests the store holds, under any key.
async fn stored_rows(pool: &PgPool) -> (i64, i64) {
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM atproto_oauth.client_session")
        .fetch_one(pool)
        .await
        .expect("the session table answers");
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM atproto_oauth.auth_request")
        .fetch_one(pool)
        .await
        .expect("the request table answers");
    (sessions, requests)
}

/// Wait until a delete of sessions is blocked on a held row in this test's
/// database, failing after 10 s.
async fn delete_waits_on_the_hold(pool: &PgPool) {
    let waiting = "SELECT count(*) FROM pg_stat_activity \
        WHERE datname = current_database() AND wait_event_type = 'Lock' \
        AND query LIKE '%DELETE FROM atproto_oauth.client_session%'";
    let blocked = async {
        loop {
            let deletes: i64 = sqlx::query_scalar(waiting)
                .fetch_one(pool)
                .await
                .expect("the activity view answers");
            if deletes > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), blocked)
        .await
        .expect("the sign-in's delete waits on the hold");
}

/// Hold every session row stored under `state` the way a slow statement
/// would, so a delete of them waits until the transaction ends.
async fn sessions_locked(world: &World, state: &str) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut lock = world.pool.begin().await.expect("a transaction");
    let hold = "SELECT 1 FROM atproto_oauth.client_session WHERE session_id = $1 FOR UPDATE";
    sqlx::query(hold)
        .bind(state)
        .fetch_all(&mut *lock)
        .await
        .expect("the rows are held");
    lock
}

/// The form fields of the latest PAR request `server` received.
async fn par_fields(server: &MockServer) -> HashMap<String, String> {
    let requests = server.received_requests().await.unwrap_or_default();
    let par = requests
        .iter()
        .rev()
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
    pool: PgPool,
    _db: test_support::pg::TestDb,
}

async fn world(authorize: &str, claimed: &str) -> World {
    world_with_fetch_timeout(authorize, claimed, Duration::from_secs(2)).await
}

/// [`world`] with each fetch cut off after `fetch` instead of 2 s.
async fn world_with_fetch_timeout(authorize: &str, claimed: &str, fetch: Duration) -> World {
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
        fetch,
        dns: Duration::from_millis(500),
    };
    let http = GuardedHttp::upstream(*server.address(), dns.clone(), timeouts);
    let resolver = Arc::new(AtprotoIdentityResolver::scripted(http, dns));

    let (pool, db) = test_support::pg::fresh_pool().await;
    let vault = || SecretVault::from_bytes(&[7u8; 32]).expect("32-byte test root key");
    let store = AtprotoAuthStore::new(pool.clone(), vault());
    let redirect_uri =
        Uri::parse("http://127.0.0.1:3000/signin-callback".to_owned()).expect("a redirect URI");
    let authenticator = AtprotoAuthenticator::new(redirect_uri, pool.clone(), vault(), resolver);
    World {
        server,
        authenticator,
        store,
        pool,
        _db: db,
    }
}

/// Start sign-in for [`HANDLE`] and return the `state` the PAR carried and
/// the browser token the browser was given.
async fn started(world: &World) -> (String, BrowserBinding) {
    let signin = world
        .authenticator
        .start(&handle(HANDLE))
        .await
        .expect("sign-in starts");
    let state = par_fields(&world.server).await["state"].clone();
    (state, signin.browser_binding)
}

/// Complete the callback for `state`, as the PDS would redirect it back to a
/// browser holding `binding`.
async fn completed(world: &World, state: &str, binding: &BrowserBinding) -> anyhow::Result<Did> {
    let presented = Some(binding.clone());
    world
        .authenticator
        .complete(
            "code".into(),
            Some(state.to_owned()),
            Some(ISSUER.into()),
            presented,
        )
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

    let url = url::Url::parse(&url.authorization_url).expect("an authorization URL");
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(url.as_str().split('?').next(), Some(AUTHORIZE));
    assert_eq!(query["request_uri"], REQUEST_URI);
    assert!(query.contains_key("client_id"));
}

/// The scope words of a space-separated scope list.
fn scope_words(scopes: &str) -> Vec<&str> {
    scopes.split_whitespace().collect()
}

/// The `scope` values in the query of the loopback client id `client_id`.
fn client_id_scopes(client_id: &str) -> Vec<String> {
    let client_id = url::Url::parse(client_id).expect("the loopback client id is a URL");
    client_id
        .query_pairs()
        .filter(|(name, _)| name == "scope")
        .map(|(_, value)| value.into_owned())
        .collect()
}

#[tokio::test]
async fn sign_in_asks_for_the_atproto_scope_alone() {
    let world = world(AUTHORIZE, HANDLE).await;

    let signin = world
        .authenticator
        .start(&handle(HANDLE))
        .await
        .expect("sign-in starts");

    let fields = par_fields(&world.server).await;
    let pushed_scope = fields.get("scope").expect("the PAR carries a scope");
    assert_eq!(
        scope_words(pushed_scope),
        ["atproto"],
        "the PAR asks for `atproto` alone"
    );
    let url = url::Url::parse(&signin.authorization_url).expect("an authorization URL");
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let client_id = query.get("client_id").expect("the URL names the client");
    let client_scopes = client_id_scopes(client_id);
    let expected_client_scopes = vec!["atproto".to_owned()];
    assert_eq!(
        client_scopes, expected_client_scopes,
        "the client id the browser carries declares `atproto` alone"
    );
    let par_mentions_transition = fields.values().any(|value| value.contains("transition"));
    let url_mentions_transition = signin.authorization_url.contains("transition");
    assert!(
        !par_mentions_transition && !url_mentions_transition,
        "no `transition` scope travels in the PAR or the authorization URL"
    );
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

    let result = authenticator.start(&handle(HANDLE)).await;

    // The 2 s fetch timeout would end it as a fetch failure, so only the start
    // deadline gives this class: no wall clock needed.
    let error = result.expect_err("a start past its deadline fails");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::Deadline)),
        "cut off by its deadline: {error:?}"
    );
}

// ---- complete ----------------------------------------------------------

#[tokio::test]
async fn the_bound_account_completes() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;

    let did = completed(&world, &state, &binding)
        .await
        .expect("the bound account signs in");

    assert_eq!(did, alice());
}

#[tokio::test]
async fn a_finished_sign_in_leaves_no_oauth_session_or_request_stored() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;

    completed(&world, &state, &binding)
        .await
        .expect("the bound account signs in");

    let stored = stored_rows(&world.pool).await;
    assert_eq!(
        stored,
        (0, 0),
        "no OAuth session and no auth request outlive the sign-in"
    );
}

#[tokio::test]
async fn a_finished_sign_in_revokes_its_refresh_token_then_its_access_token() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;

    completed(&world, &state, &binding)
        .await
        .expect("the bound account signs in");

    let revoked = revocations_arrived(&world.server, 2).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned(), ACCESS_TOKEN.to_owned()];
    assert_eq!(
        revoked, expected_revoked,
        "the issued refresh token is revoked first, then the access token"
    );
}

#[tokio::test]
async fn a_finished_sign_in_sends_its_revocation_only_after_its_session_is_deleted() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    // Another row under the same state, held: the sign-in's delete of every
    // session under its state waits on it until the hold ends.
    let held = saved_session(&mallory(), &state);
    world.store.upsert_session(held).await.expect("saved");
    let hold = sessions_locked(&world, &state).await;

    let answered = std::cell::Cell::new(false);
    let signing_in = async {
        let signed_in = completed(&world, &state, &binding).await;
        answered.set(true);
        signed_in
    };
    let while_deleting = async {
        delete_waits_on_the_hold(&world.pool).await;
        let answered_while_deleting = answered.get();
        let session = world
            .store
            .get_session(&at_did(&alice()), &state)
            .await
            .expect("the store answers");
        let revocations = hits(&world.server, AUTH, "/oauth/revoke").await;
        hold.rollback().await.expect("the hold ends");
        (answered_while_deleting, session.is_some(), revocations)
    };
    let (signed_in, (answered_while_deleting, saved_while_deleting, revoked_while_deleting)) =
        tokio::join!(signing_in, while_deleting);

    assert!(
        !answered_while_deleting,
        "the sign-in does not answer before its delete is done"
    );
    assert!(
        saved_while_deleting,
        "the callback saved its session and was held at the delete"
    );
    assert_eq!(
        revoked_while_deleting, 0,
        "nothing is revoked while the delete is pending"
    );
    let did = signed_in.expect("the bound account signs in");
    assert_eq!(did, alice());
    let session = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the session is deleted");
    let revoked = revocations_arrived(&world.server, 2).await;
    assert_eq!(revoked.len(), 2, "and then revoked: {revoked:?}");
}

#[tokio::test]
async fn a_delete_that_runs_out_of_time_still_signs_in_and_still_revokes() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let held = saved_session(&mallory(), &state);
    world.store.upsert_session(held).await.expect("saved");
    // Held for the whole callback: the delete cannot finish in time.
    let hold = sessions_locked(&world, &state).await;
    let authenticator = world.authenticator.with_cleanup(Duration::from_millis(300));

    let signed_in = authenticator
        .complete(
            "code".into(),
            Some(state.clone()),
            Some(ISSUER.into()),
            Some(binding.clone()),
        )
        .await;
    let revoked = revocations_arrived(&world.server, 2).await;
    hold.rollback().await.expect("the hold ends");

    let did = signed_in.expect("a failed delete does not fail the sign-in");
    assert_eq!(did, alice());
    let expected_revoked = vec![REFRESH_TOKEN.to_owned(), ACCESS_TOKEN.to_owned()];
    assert_eq!(revoked, expected_revoked, "the tokens are still revoked");
}

#[tokio::test]
async fn a_revocation_that_never_answers_does_not_hold_up_the_sign_in() {
    // A 30 s fetch timeout and a 30 s revocation deadline: only the sign-in
    // not waiting for the revocation can return within 5 s.
    let world = world_with_fetch_timeout(AUTHORIZE, HANDLE, Duration::from_secs(30)).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let never = ResponseTemplate::new(204).set_delay(Duration::from_secs(120));
    mount_with(&world.server, ("POST", AUTH, "/oauth/revoke"), never, true).await;
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_secs(10), Duration::from_secs(30));

    let begun = std::time::Instant::now();
    let signed_in = authenticator
        .complete(
            "code".into(),
            Some(state.clone()),
            Some(ISSUER.into()),
            Some(binding.clone()),
        )
        .await;
    let waited = begun.elapsed();

    let did = signed_in.expect("the bound account signs in");
    assert_eq!(did, alice());
    assert!(
        waited < Duration::from_secs(5),
        "the sign-in did not wait for the revocation: {waited:?}"
    );
    let revoked = revocations_arrived(&world.server, 1).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned()];
    assert_eq!(revoked, expected_revoked, "the revocation was sent");
    let stored = stored_rows(&world.pool).await;
    assert_eq!(stored, (0, 0), "nothing is stored meanwhile");
}

#[tokio::test]
async fn a_refused_scope_is_answered_without_waiting_for_its_revocation() {
    // As above: only a refusal that does not wait for the revocation can
    // return within 5 s.
    let world = world_with_fetch_timeout(AUTHORIZE, HANDLE, Duration::from_secs(30)).await;
    serve_token_granting(&world.server, &alice(), Some("atproto transition:generic")).await;
    let (state, binding) = started(&world).await;
    let never = ResponseTemplate::new(204).set_delay(Duration::from_secs(120));
    mount_with(&world.server, ("POST", AUTH, "/oauth/revoke"), never, true).await;
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_secs(10), Duration::from_secs(30));

    let begun = std::time::Instant::now();
    let refused = authenticator
        .complete(
            "code".into(),
            Some(state.clone()),
            Some(ISSUER.into()),
            Some(binding.clone()),
        )
        .await;
    let waited = begun.elapsed();

    let error = refused.expect_err("the wider grant is refused");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::GrantedScope)),
        "refused for its granted scope: {error:?}"
    );
    assert!(
        waited < Duration::from_secs(5),
        "the refusal did not wait for the revocation: {waited:?}"
    );
    let revoked = revocations_arrived(&world.server, 1).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned()];
    assert_eq!(revoked, expected_revoked, "the revocation was sent");
}

/// Sign in as the bound account against a revocation endpoint answering
/// with `answer`, and return the `info` lines and above logged until both
/// revocations were answered.
async fn revocation_log_after_sign_in(answer: ResponseTemplate) -> Vec<String> {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    mount_with(&world.server, ("POST", AUTH, "/oauth/revoke"), answer, true).await;

    let (signed_in, events) = record_events(async {
        let signed_in = completed(&world, &state, &binding).await;
        revocations_arrived(&world.server, 2).await;
        // The background task logs once each answer is in.
        tokio::time::sleep(Duration::from_millis(500)).await;
        signed_in
    })
    .await;

    signed_in.expect("the revocation's answer does not change the sign-in's");
    let logged = lines_at(&events, tracing::Level::INFO, true);
    for line in &logged {
        for secret in [REFRESH_TOKEN, ACCESS_TOKEN, state.as_str(), "did:plc"] {
            assert!(
                !line.contains(secret),
                "an info line carries {secret:?}: {line}"
            );
        }
    }
    logged
}

#[tokio::test]
async fn a_failed_revocation_after_a_sign_in_is_logged_as_a_revocation_by_class_only() {
    let refusals = [
        ResponseTemplate::new(500),
        ResponseTemplate::new(400).set_body_json(json!({ "error": "invalid_request" })),
    ];

    for refusal in refusals {
        let logged = revocation_log_after_sign_in(refusal).await;

        let failed_revocation =
            "message=revocation failed failure=revocation request failed ".to_string();
        let expected_logged = vec![failed_revocation.clone(), failed_revocation];
        assert_eq!(
            logged, expected_logged,
            "one line per token, by class, and never as a failed sign-in"
        );
    }
}

#[tokio::test]
async fn a_revocation_answered_with_200_or_204_is_a_revocation() {
    let answers = [
        ResponseTemplate::new(200).set_body_json(json!({})),
        ResponseTemplate::new(204),
    ];

    for answer in answers {
        let logged = revocation_log_after_sign_in(answer).await;

        assert!(logged.is_empty(), "nothing failed: {logged:?}");
    }
}

/// Whether the sign-in's store still holds a refused grant under `state`.
fn still_held(world: &World, state: &str) -> bool {
    let store = &world.authenticator.signin.oauth.registry.store;
    store.holds_refused(state)
}

/// Complete a sign-in for the bound account whose token response grants
/// `scope`, and check it is refused, never stored, and revoked.
async fn a_grant_of(scope: Option<&str>) {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token_granting(&world.server, &alice(), scope).await;
    let (state, binding) = started(&world).await;

    let result = completed(&world, &state, &binding).await;

    let error = result.expect_err("the grant is refused");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::GrantedScope)),
        "refused for its granted scope: {error:?}"
    );
    assert!(
        !is_browser_mismatch(&error) && !error.is::<AccountMismatch>(),
        "a refused scope stays generic at the port: {error:?}"
    );
    let session = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the refused session row is gone");
    assert!(
        !still_pending(&world, &state).await,
        "and so is its request"
    );
    let stored = stored_rows(&world.pool).await;
    assert_eq!(stored, (0, 0), "the refused grant was never stored");
    assert!(
        !still_held(&world, &state),
        "no refused grant is left in memory"
    );
    let revoked = revocations_arrived(&world.server, 2).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned(), ACCESS_TOKEN.to_owned()];
    assert_eq!(
        revoked, expected_revoked,
        "the refused grant's refresh token is revoked, then its access token"
    );
}

#[tokio::test]
async fn a_grant_wider_than_atproto_is_refused_never_stored_and_revoked() {
    a_grant_of(Some("atproto transition:generic")).await;
}

#[tokio::test]
async fn a_grant_naming_no_scope_is_refused_never_stored_and_revoked() {
    a_grant_of(None).await;
}

#[tokio::test]
async fn another_account_with_a_wider_grant_is_refused_as_a_mismatch_and_revoked() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token_granting(
        &world.server,
        &mallory(),
        Some("atproto transition:generic"),
    )
    .await;
    let (state, binding) = started(&world).await;

    let result = completed(&world, &state, &binding).await;

    let error = result.expect_err("another account is refused");
    assert!(
        error.is::<AccountMismatch>(),
        "the account check runs before the scope check: {error:?}"
    );
    let revoked = revoked_tokens(&world.server).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned(), ACCESS_TOKEN.to_owned()];
    assert_eq!(
        revoked, expected_revoked,
        "the mismatched wider grant is revoked, refresh token first"
    );
    let stored = stored_rows(&world.pool).await;
    assert_eq!(stored, (0, 0), "the wider grant was never stored");
    assert!(
        !still_held(&world, &state),
        "no refused grant is left in memory"
    );
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the mismatched session row is gone");
}

#[tokio::test]
async fn another_account_is_revoked_deleted_and_refused() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &mallory()).await;
    let (state, binding) = started(&world).await;

    let result = completed(&world, &state, &binding).await;

    let error = result.expect_err("another account is refused");
    assert!(
        error.is::<AccountMismatch>(),
        "refused as an account mismatch"
    );
    let revoked = revoked_tokens(&world.server).await;
    let expected_revoked = vec![REFRESH_TOKEN.to_owned(), ACCESS_TOKEN.to_owned()];
    assert_eq!(
        revoked, expected_revoked,
        "the mismatched refresh token is revoked first, then the access token"
    );
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
    let (state, binding) = started(&world).await;
    // The authorization server vanishes once the callback is through: its two
    // metadata fetches (the callback's own, the issuer check's) answer, and
    // the revocation's cannot, so the session must be deleted without a revocation.
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

    let result = completed(&world, &state, &binding).await;

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
    let (state, binding) = started(&world).await;
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
    // Re-saved bound to the same browser, so the browser check passes and the
    // missing DID is what refuses it.
    let browser_hash = token_hash(binding.as_ref());
    world
        .store
        .save_bound_auth_request(&unbound, &browser_hash)
        .await
        .expect("save");

    let result = completed(&world, &state, &binding).await;

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

    let unissued = BrowserBinding::from("never-issued".to_owned());
    let result = completed(&world, "never-issued", &unissued).await;

    assert!(result.is_err());
    assert_eq!(hits(&world.server, AUTH, "/oauth/token").await, 0);
}

#[tokio::test]
async fn the_issuer_check_refuses_a_did_web_with_a_path() {
    let world = world(AUTHORIZE, HANDLE).await;
    let with_path = Did::from("did:web:auth.example.com:users:mallory".to_owned());
    serve_token(&world.server, &with_path).await;
    let (state, binding) = started(&world).await;

    let result = completed(&world, &state, &binding).await;

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
    let (state, binding) = started(&world).await;

    let result = completed(&world, &state, &binding).await;

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
        scope: Some(SmolStr::new_static(OAUTH_SCOPES)),
        refresh_token: Some(SmolStr::new_static(REFRESH_TOKEN)),
        access_token: SmolStr::new_static(ACCESS_TOKEN),
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
    // A 30 s fetch timeout, so only the 300 ms revocation deadline can end the
    // stall below within the 10 s bound, however loaded the machine.
    let world = world_with_fetch_timeout(AUTHORIZE, HANDLE, Duration::from_secs(30)).await;
    serve_token(&world.server, &mallory()).await;
    let (state, binding) = started(&world).await;
    // The authorization server answers the callback, then stalls the
    // revocation's metadata fetch: the stall must neither strand the row nor
    // change the answer.
    let stalled = ResponseTemplate::new(200)
        .set_body_json(server_metadata(AUTHORIZE))
        .set_delay(Duration::from_secs(60));
    metadata_then(&world.server, stalled).await;
    // The callback's deadline is only room for the callback, never the point.
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_secs(10), Duration::from_millis(300));

    let begun = std::time::Instant::now();
    let refusing = authenticator.complete(
        "code".into(),
        Some(state.clone()),
        Some(ISSUER.into()),
        Some(binding.clone()),
    );
    let (result, events) = record_events(refusing).await;

    let error = result.expect_err("another account is refused");
    assert!(error.is::<AccountMismatch>(), "still an account mismatch");
    assert!(
        begun.elapsed() < Duration::from_secs(10),
        "the revocation gave up"
    );
    let info = lines_at(&events, tracing::Level::INFO, false);
    let expected_info = vec![
        "message=revocation failed failure=revocation deadline passed ".to_string(),
        "message=sign-in failed failure=signed in as a different account ".to_string(),
    ];
    assert_eq!(info, expected_info, "given up at the revocation deadline");
    let session = world
        .store
        .get_session(&at_did(&mallory()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "the row is gone before any revocation");
}

#[tokio::test]
async fn an_authorization_server_without_a_revocation_endpoint_is_logged_by_class() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &mallory()).await;
    let (state, binding) = started(&world).await;
    // The callback's two metadata fetches see the full metadata; the
    // revocation's sees none of a revocation endpoint.
    let mut without_revocation = server_metadata(AUTHORIZE);
    without_revocation
        .as_object_mut()
        .expect("the metadata is an object")
        .remove("revocation_endpoint");
    let later = ResponseTemplate::new(200).set_body_json(without_revocation);
    metadata_then(&world.server, later).await;

    let (result, events) = record_events(completed(&world, &state, &binding)).await;

    let error = result.expect_err("another account is refused");
    assert!(error.is::<AccountMismatch>(), "still an account mismatch");
    let info = lines_at(&events, tracing::Level::INFO, false);
    let expected_info = vec![
        "message=revocation failed failure=no revocation endpoint ".to_string(),
        "message=sign-in failed failure=signed in as a different account ".to_string(),
    ];
    assert_eq!(info, expected_info, "the missing endpoint is logged once");
    let revoked = revoked_tokens(&world.server).await;
    assert!(revoked.is_empty(), "nothing is sent: {revoked:?}");
}

#[tokio::test]
async fn the_complete_deadline_cuts_off_the_callback_and_discards_its_session() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    // A token exchange that never answers in time, and a session already saved
    // under this sign-in's state, as if the callback had got that far.
    let slow = ResponseTemplate::new(200).set_delay(Duration::from_secs(5));
    mount(&world.server, "POST", AUTH, "/oauth/token", slow).await;
    let saved = saved_session(&mallory(), &state);
    world.store.upsert_session(saved).await.expect("saved");
    // Long enough for the claim on a loaded machine, and shorter than the 2 s
    // fetch timeout, which starts only after the claim: the deadline always
    // ends the exchange first.
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_millis(1500), Duration::from_secs(5));

    let result = authenticator
        .complete(
            "code".into(),
            Some(state.clone()),
            Some(ISSUER.into()),
            Some(binding.clone()),
        )
        .await;

    let error = result.expect_err("a callback past its deadline fails");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::Deadline)),
        "cut off by its deadline, not the fetch timeout: {error:?}"
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
    let (state, binding) = started(&world).await;
    // The token exchange for another account answers after the request is gone.
    let tokens = json!({
        "access_token": ACCESS_TOKEN,
        "token_type": "DPoP",
        "expires_in": 3600,
        "scope": OAUTH_SCOPES,
        "sub": mallory().as_ref(),
    });
    let slow_tokens = ResponseTemplate::new(200)
        .set_body_json(tokens)
        .set_delay(Duration::from_millis(500));
    mount(&world.server, "POST", AUTH, "/oauth/token", slow_tokens).await;

    let callback = world.authenticator.complete(
        "code".into(),
        Some(state.clone()),
        Some(ISSUER.into()),
        Some(binding.clone()),
    );
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
    let (state, binding) = started(&mismatched).await;

    let ((), events) = record_events(async {
        let typed = handle(HANDLE);
        let _ = unconfirmed.authenticator.start(&typed).await;
        let _ = refused_endpoint.authenticator.start(&typed).await;
        let _ = completed(&mismatched, &state, &binding).await;
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
async fn a_replayed_callback_is_refused_and_stores_nothing() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    completed(&world, &state, &binding)
        .await
        .expect("the first callback signs in");

    // A reload, the back button, or anyone who saw the callback URL.
    let replay = completed(&world, &state, &binding).await;

    assert!(replay.is_err(), "a state is single-use");
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        1,
        "the replay exchanged no code"
    );
    let stored = stored_rows(&world.pool).await;
    assert_eq!(stored, (0, 0), "nothing is stored after either callback");
}

// ---- the browser binding ---------------------------------------------------

/// Complete the callback for `state` from a browser presenting `presented`.
async fn completed_presenting(
    world: &World,
    state: &str,
    presented: Option<BrowserBinding>,
) -> anyhow::Result<Did> {
    world
        .authenticator
        .complete(
            "code".into(),
            Some(state.to_owned()),
            Some(ISSUER.into()),
            presented,
        )
        .await
}

/// Whether `state`'s auth request is still stored.
async fn still_pending(world: &World, state: &str) -> bool {
    let request = world.store.get_auth_req_info(state).await.expect("answers");
    request.is_some()
}

#[tokio::test]
async fn only_a_hash_of_the_browser_token_is_stored() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;

    let stored: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT browser_binding FROM atproto_oauth.auth_request WHERE state = $1",
    )
    .bind(&state)
    .fetch_one(&world.pool)
    .await
    .expect("the request row");

    let token = binding.as_ref();
    assert!(token.len() >= 43, "at least 256 bits, base64url");
    let expected_hash = Sha256::digest(token.as_bytes()).to_vec();
    assert_eq!(stored, Some(expected_hash), "the hash, never the token");
}

#[tokio::test]
async fn a_callback_from_a_browser_without_the_cookie_is_refused() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, _binding) = started(&world).await;

    let result = completed_presenting(&world, &state, None).await;

    assert!(result.is_err(), "no cookie, no sign-in");
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "no code exchanged"
    );
    assert!(
        !still_pending(&world, &state).await,
        "the refused request is deleted"
    );
}

#[tokio::test]
async fn a_callback_carrying_another_sign_ins_cookie_is_refused() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    // The attacker's sign-in, approved at their PDS but never finished...
    let (attackers_state, _attackers_binding) = started(&world).await;
    // ...opened in a victim's browser, which holds a cookie from its own sign-in.
    let (_victims_state, victims_binding) = started(&world).await;

    let result = completed_presenting(&world, &attackers_state, Some(victims_binding)).await;

    assert!(result.is_err(), "another sign-in's cookie does not fit");
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "no code exchanged"
    );
}

#[tokio::test]
async fn a_callback_with_a_wrong_token_is_refused() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let mut forged = binding.as_ref().to_owned();
    let flipped = if forged.ends_with('A') { 'B' } else { 'A' };
    forged.pop();
    forged.push(flipped);

    let result = completed_presenting(&world, &state, Some(BrowserBinding::from(forged))).await;

    assert!(result.is_err(), "a token one character off is refused");
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "no code exchanged"
    );
    assert!(
        !still_pending(&world, &state).await,
        "the refused request is deleted"
    );
}

#[tokio::test]
async fn an_expired_sign_in_is_refused_even_with_its_own_cookie() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let aged = "UPDATE atproto_oauth.auth_request SET created_at = now() - interval '11 minutes' WHERE state = $1";
    sqlx::query(aged)
        .bind(&state)
        .execute(&world.pool)
        .await
        .expect("aged");

    let result = completed(&world, &state, &binding).await;

    assert!(result.is_err(), "past its lifetime the sign-in is gone");
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "no code exchanged"
    );
}

#[tokio::test]
async fn the_cookie_lives_exactly_as_long_as_the_sign_in() {
    let world = world(AUTHORIZE, HANDLE).await;
    let signin = world
        .authenticator
        .start(&handle(HANDLE))
        .await
        .expect("sign-in starts");
    assert_eq!(signin.lifetime, SIGNIN_LIFETIME);
    assert!(SIGNIN_LIFETIME <= Duration::from_secs(10 * 60));
}

#[test]
fn hashes_match_only_on_every_byte() {
    let hash = token_hash("token");
    assert!(hashes_match(&hash, &hash));
    let mut last_byte_off = hash;
    last_byte_off[31] ^= 1;
    assert!(!hashes_match(&hash, &last_byte_off));
    assert!(
        !hashes_match(&hash, &hash[..31]),
        "a short value never matches"
    );
    assert!(!hashes_match(&hash, &[]), "an empty value never matches");
}

#[test]
fn every_browser_token_is_fresh() {
    let first = new_browser_token();
    let second = new_browser_token();
    assert_ne!(first, second);
    assert_eq!(first.len(), 43, "32 bytes, base64url without padding");
}

// ---- the claim -------------------------------------------------------------

/// Hold `state`'s request row the way a winning callback's claim does while its
/// statement is in flight: the binding cleared, not yet committed.
async fn claim_in_flight(world: &World, state: &str) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut claim = world.pool.begin().await.expect("a transaction");
    let clear = "UPDATE atproto_oauth.auth_request SET browser_binding = NULL WHERE state = $1";
    sqlx::query(clear)
        .bind(state)
        .execute(&mut *claim)
        .await
        .expect("the claim holds the row");
    claim
}

/// The sign-in failure behind a refused `complete`.
fn failure_of(error: &anyhow::Error) -> Option<&SigninFailure> {
    error.downcast_ref::<SigninFailure>()
}

#[tokio::test]
async fn exactly_one_of_two_racing_claims_wins() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    let bound = token_hash(binding.as_ref());

    let stale = world
        .store
        .claim_browser_binding(&state, &[0u8; 32], SIGNIN_LIFETIME)
        .await
        .expect("the store answers");
    let (first, second) = tokio::join!(
        world
            .store
            .claim_browser_binding(&state, &bound, SIGNIN_LIFETIME),
        world
            .store
            .claim_browser_binding(&state, &bound, SIGNIN_LIFETIME),
    );

    assert!(!stale, "a hash that isn't the binding claims nothing");
    let claims = [
        first.expect("the store answers"),
        second.expect("the store answers"),
    ];
    let won = claims.iter().filter(|claimed| **claimed).count();
    assert_eq!(won, 1, "exactly one claim wins");
}

#[tokio::test]
async fn a_callback_that_loses_the_claim_exchanges_nothing_and_leaves_the_winner_alone() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let winners = saved_session(&alice(), &state);
    world.store.upsert_session(winners).await.expect("saved");
    let winning_claim = claim_in_flight(&world, &state).await;

    // The loser reads the live binding, then waits on the row until the
    // winner's claim commits.
    let commit_later = async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        winning_claim
            .commit()
            .await
            .expect("the winner's claim commits");
    };
    let (losing, ()) = tokio::join!(completed(&world, &state, &binding), commit_later);

    let error = losing.expect_err("the loser is refused");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::Claimed)),
        "refused as already claimed: {error:?}"
    );
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "the loser exchanged no code"
    );
    assert!(
        still_pending(&world, &state).await,
        "the request is left to the winner"
    );
    let session = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_some(), "the winner's session survives the loser");
}

#[tokio::test]
async fn a_callback_after_the_claim_is_refused_and_leaves_the_request_to_the_claimer() {
    let world = world(AUTHORIZE, HANDLE).await;
    serve_token(&world.server, &alice()).await;
    let (state, binding) = started(&world).await;
    let claimed = world
        .store
        .claim_browser_binding(&state, &token_hash(binding.as_ref()), SIGNIN_LIFETIME)
        .await
        .expect("the store answers");
    assert!(claimed, "the first callback claims the sign-in");

    let with_the_cookie = completed(&world, &state, &binding).await;
    let without_a_cookie = completed_presenting(&world, &state, None).await;

    for refused in [with_the_cookie, without_a_cookie] {
        let error = refused.expect_err("a claimed sign-in is the claimer's");
        assert!(
            matches!(failure_of(&error), Some(SigninFailure::Claimed)),
            "refused as already claimed: {error:?}"
        );
    }
    assert_eq!(
        hits(&world.server, AUTH, "/oauth/token").await,
        0,
        "no code exchanged"
    );
    assert!(
        still_pending(&world, &state).await,
        "the request is left to the claimer"
    );
}

#[test]
fn a_row_reads_as_bound_claimed_or_gone() {
    let hash = vec![7u8; 32];
    assert_eq!(
        StoredBinding::from(Some(Some(hash.clone()))),
        StoredBinding::Bound(hash)
    );
    assert_eq!(StoredBinding::from(Some(None)), StoredBinding::Claimed);
    assert_eq!(StoredBinding::from(None), StoredBinding::Gone);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_racing_a_claim_leaves_the_claimed_request() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, _binding) = started(&world).await;
    let claim = claim_in_flight(&world, &state).await;
    let commit_later = async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        claim.commit().await.expect("the claim commits");
    };
    let forged = Some(BrowserBinding::from("A".repeat(43)));
    let (refused, ()) = tokio::join!(completed_presenting(&world, &state, forged), commit_later);

    assert!(refused.is_err(), "a wrong cookie is refused");
    assert!(
        still_pending(&world, &state).await,
        "the claimed request is the claimer's"
    );
}

/// Age `state`'s request past the sign-in's lifetime.
async fn aged_out(world: &World, state: &str) {
    let aged = "UPDATE atproto_oauth.auth_request SET created_at = now() - interval '11 minutes' WHERE state = $1";
    sqlx::query(aged)
        .bind(state)
        .execute(&world.pool)
        .await
        .expect("aged");
}

#[tokio::test]
async fn a_late_callback_leaves_a_claimed_request_past_its_lifetime() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    let claimed = world
        .store
        .claim_browser_binding(&state, &token_hash(binding.as_ref()), SIGNIN_LIFETIME)
        .await
        .expect("the store answers");
    assert!(claimed, "the first callback claims the sign-in");
    aged_out(&world, &state).await;

    let late = completed(&world, &state, &binding).await;

    let error = late.expect_err("a sign-in past its lifetime is refused");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::StaleSignin)),
        "refused as stale: {error:?}"
    );
    assert!(
        still_pending(&world, &state).await,
        "a claimed request is never a refusal's to delete"
    );
}

#[tokio::test]
async fn a_request_past_its_lifetime_cannot_be_claimed() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    aged_out(&world, &state).await;

    let claimed = world
        .store
        .claim_browser_binding(&state, &token_hash(binding.as_ref()), SIGNIN_LIFETIME)
        .await
        .expect("the store answers");

    assert!(!claimed, "the claim checks the age itself");
}

#[tokio::test]
async fn the_claimers_cleanup_deletes_its_request_and_sessions() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    let claimed = world
        .store
        .claim_browser_binding(&state, &token_hash(binding.as_ref()), SIGNIN_LIFETIME)
        .await
        .expect("the store answers");
    assert!(claimed, "this attempt owns the sign-in");
    let saved = saved_session(&alice(), &state);
    world.store.upsert_session(saved).await.expect("saved");

    world
        .authenticator
        .signin
        .discard(Some(&state), Duration::from_secs(5))
        .await;

    assert!(
        !still_pending(&world, &state).await,
        "no orphan request outlives a failed claimer"
    );
    let session = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(session.is_none(), "and no session either");
}

#[tokio::test]
async fn a_bound_request_is_never_saved_over_another_under_the_same_state() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, _binding) = started(&world).await;
    let request = world
        .store
        .get_auth_req_info(&state)
        .await
        .expect("the store answers")
        .expect("the auth request is stored");

    let resaved = world
        .store
        .save_bound_auth_request(&request, &[9u8; 32])
        .await;

    assert!(resaved.is_err(), "a stored state is never overwritten");
    let stored: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT browser_binding FROM atproto_oauth.auth_request WHERE state = $1",
    )
    .bind(&state)
    .fetch_one(&world.pool)
    .await
    .expect("the request row");
    assert_ne!(stored, Some(vec![9u8; 32]), "the first binding stands");
}

#[tokio::test]
async fn a_callback_stalled_before_its_claim_deletes_nothing_at_its_deadline() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    let winners = saved_session(&alice(), &state);
    world.store.upsert_session(winners).await.expect("saved");
    // The winner holds the row mid-claim, so this callback stalls on its own
    // claim until its deadline passes.
    let winning_claim = claim_in_flight(&world, &state).await;
    let authenticator = world
        .authenticator
        .with_deadlines(Duration::from_millis(300), Duration::from_secs(5));

    let result = authenticator
        .complete(
            "code".into(),
            Some(state.clone()),
            Some(ISSUER.into()),
            Some(binding.clone()),
        )
        .await;
    winning_claim
        .rollback()
        .await
        .expect("the winner's claim lets go");

    let error = result.expect_err("a callback past its deadline fails");
    assert!(
        matches!(failure_of(&error), Some(SigninFailure::Deadline)),
        "cut off by its deadline: {error:?}"
    );
    let session = world
        .store
        .get_session(&at_did(&alice()), &state)
        .await
        .expect("the store answers");
    assert!(
        session.is_some(),
        "an attempt that never claimed deletes nothing"
    );
}

// ---- the port's refusal for the wrong browser ------------------------------

/// Whether `refused` reaches the port as the domain's browser refusal.
fn is_browser_mismatch(refused: &anyhow::Error) -> bool {
    refused.is::<domain::ports::BrowserMismatch>()
}

#[tokio::test]
async fn a_callback_without_the_cookie_reaches_the_port_as_a_browser_mismatch() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, _binding) = started(&world).await;

    let refused = completed_presenting(&world, &state, None)
        .await
        .expect_err("no cookie, no sign-in");

    assert!(is_browser_mismatch(&refused), "{refused:?}");
    assert!(
        matches!(failure_of(&refused), Some(SigninFailure::NoBrowserBinding)),
        "the class is kept: {refused:?}"
    );
}

#[tokio::test]
async fn an_expired_sign_in_reaches_the_port_as_a_browser_mismatch() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    aged_out(&world, &state).await;

    let refused = completed(&world, &state, &binding)
        .await
        .expect_err("past its lifetime the sign-in is gone");

    assert!(is_browser_mismatch(&refused), "{refused:?}");
    assert!(
        matches!(failure_of(&refused), Some(SigninFailure::StaleSignin)),
        "the class is kept: {refused:?}"
    );
}

#[tokio::test]
async fn another_sign_ins_cookie_reaches_the_port_as_a_browser_mismatch() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (attackers_state, _attackers_binding) = started(&world).await;
    let (_victims_state, victims_binding) = started(&world).await;

    let refused = completed(&world, &attackers_state, &victims_binding)
        .await
        .expect_err("another sign-in's cookie does not fit");

    assert!(is_browser_mismatch(&refused), "{refused:?}");
    assert!(
        matches!(failure_of(&refused), Some(SigninFailure::BrowserMismatch)),
        "the class is kept: {refused:?}"
    );
}

#[tokio::test]
async fn a_claimed_sign_in_is_not_a_browser_mismatch() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (state, binding) = started(&world).await;
    let claimed = world
        .store
        .claim_browser_binding(&state, &token_hash(binding.as_ref()), SIGNIN_LIFETIME)
        .await
        .expect("the store answers");
    assert!(claimed, "the first callback claims the sign-in");

    let refused = completed(&world, &state, &binding)
        .await
        .expect_err("a claimed sign-in is the claimer's");

    assert!(!is_browser_mismatch(&refused), "{refused:?}");
    assert!(
        matches!(failure_of(&refused), Some(SigninFailure::Claimed)),
        "refused as already claimed: {refused:?}"
    );
}

#[tokio::test]
async fn a_callback_without_a_state_is_not_a_browser_mismatch() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (_state, binding) = started(&world).await;

    let refused = world
        .authenticator
        .complete("code".into(), None, Some(ISSUER.into()), Some(binding))
        .await
        .expect_err("no state, no sign-in");

    assert!(!is_browser_mismatch(&refused), "{refused:?}");
    assert!(
        matches!(failure_of(&refused), Some(SigninFailure::UnknownState)),
        "refused as an unknown state: {refused:?}"
    );
}

#[tokio::test]
async fn each_browser_refusal_logs_its_own_class_at_info() {
    let world = world(AUTHORIZE, HANDLE).await;
    let (no_cookie_state, other_binding) = started(&world).await;
    let (expired_state, expired_binding) = started(&world).await;
    let (wrong_cookie_state, _binding) = started(&world).await;
    aged_out(&world, &expired_state).await;

    let ((), events) = record_events(async {
        let _ = completed_presenting(&world, &no_cookie_state, None).await;
        let _ = completed(&world, &expired_state, &expired_binding).await;
        let _ = completed(&world, &wrong_cookie_state, &other_binding).await;
    })
    .await;

    let info = lines_at(&events, tracing::Level::INFO, false);
    let expected_info = vec![
        "message=sign-in failed failure=no browser binding ".to_string(),
        "message=sign-in failed failure=no live bound sign-in ".to_string(),
        "message=sign-in failed failure=browser binding mismatch ".to_string(),
    ];
    assert_eq!(info, expected_info);
    for line in lines_at(&events, tracing::Level::INFO, true) {
        for state in [&no_cookie_state, &expired_state, &wrong_cookie_state] {
            assert!(
                !line.contains(state.as_str()),
                "an info line carries a state: {line}"
            );
        }
    }
}
