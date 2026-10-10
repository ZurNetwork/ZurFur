use std::time::Duration;

use serde_json::json;
use test_support::identity_resolver_contract::{StagedIdentities, identity_resolver_contract};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::guarded_http::{Script, ScriptedLookup, Timeouts, TxtScript};
use crate::resolver::log_recorder::{lines_at, record_events};

/// Every staged host answers with this public address; the upstream route then
/// delivers the request to the mock.
const PUBLIC: &str = "93.184.216.34";
/// A forbidden address: a host answering with it is refused at connect.
const PRIVATE: &str = "10.0.0.1";
/// The AppView a resolver with a Bluesky fallback would ask.
const BLUESKY_APPVIEW: &str = "public.api.bsky.app";

fn plc(fill: char) -> Did {
    let id: String = std::iter::repeat_n(fill, 24).collect();
    Did::from(format!("did:plc:{id}"))
}

fn handle(text: &str) -> AtHandle {
    text.parse().expect("a valid handle")
}

fn resolves_to(address: &str) -> Script {
    let address = address.parse().expect("a test address");
    Script::Answer(vec![address])
}

fn records(values: &[&str]) -> TxtScript {
    let values = values.iter().map(|value| (*value).to_string()).collect();
    TxtScript::Records(values)
}

fn did_record(did: &Did) -> TxtScript {
    records(&[&format!("did={did}")])
}

fn txt_at(handle: &str) -> String {
    format!("_atproto.{handle}.")
}

fn fqdn(host: &str) -> String {
    format!("{host}.")
}

/// A DID document for `did` claiming `also_known_as`.
fn document(did: &Did, also_known_as: &[&str]) -> serde_json::Value {
    let also_known_as: Vec<String> = also_known_as
        .iter()
        .map(|handle| format!("at://{handle}"))
        .collect();
    json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did.as_ref(),
        "alsoKnownAs": also_known_as,
        "verificationMethod": [],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": "https://pds.example.com",
        }],
    })
}

/// Answer `GET https://<host><route>` with `response`.
async fn serve(server: &MockServer, host: &str, route: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(header("host", host))
        .and(path(route))
        .respond_with(response)
        .mount(server)
        .await;
}

/// Serve `did`'s document from the PLC directory with `response`.
async fn serve_plc(server: &MockServer, did: &Did, response: ResponseTemplate) {
    serve(server, "plc.directory", &format!("/{did}"), response).await;
}

/// Serve the document `did` claiming `also_known_as` from the PLC directory.
async fn serve_plc_document(server: &MockServer, did: &Did, also_known_as: &[&str]) {
    let body = document(did, also_known_as);
    serve_plc(server, did, ResponseTemplate::new(200).set_body_json(body)).await;
}

/// Serve `host`'s handle file with `response`.
async fn serve_well_known(server: &MockServer, host: &str, response: ResponseTemplate) {
    serve(server, host, "/.well-known/atproto-did", response).await;
}

/// How many requests carrying `Host: host` reached `server`.
async fn requests_to(server: &MockServer, host: &str) -> usize {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| {
            request
                .headers
                .get("host")
                .and_then(|value| value.to_str().ok())
                == Some(host)
        })
        .count()
}

async fn requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap_or_default().len()
}

/// Deadlines short enough that a hang fails the test quickly.
fn test_deadlines() -> Deadlines {
    Deadlines {
        port_call: Duration::from_secs(5),
        dns: Duration::from_millis(200),
    }
}

/// The resolver over `dns` (plus the PLC directory) and the upstream route to
/// `server`, with `deadlines`.
fn resolver_with(
    server: &MockServer,
    dns: ScriptedLookup,
    deadlines: Deadlines,
) -> (AtprotoIdentityResolver, Arc<ScriptedLookup>) {
    let dns = Arc::new(dns.with("plc.directory.", resolves_to(PUBLIC)));
    let timeouts = Timeouts {
        fetch: Duration::from_secs(1),
        dns: Duration::from_millis(200),
    };
    let http = GuardedHttp::upstream(*server.address(), dns.clone(), timeouts);
    let resolver = AtprotoIdentityResolver::over(http, dns.clone(), deadlines);
    (resolver, dns)
}

fn resolver_over(
    server: &MockServer,
    dns: ScriptedLookup,
) -> (AtprotoIdentityResolver, Arc<ScriptedLookup>) {
    resolver_with(server, dns, test_deadlines())
}

// --- handle → DID ---

#[tokio::test]
async fn a_handle_with_only_a_txt_record_resolves() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["alice.example.com"]).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), did_record(&alice));
    let (resolver, _) = resolver_over(&server, dns);

    let did = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert_eq!(did.ok(), Some(alice));
}

#[tokio::test]
async fn a_handle_with_only_a_well_known_file_resolves() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["alice.example.com"]).await;
    let file = ResponseTemplate::new(200).set_body_string(format!("  {alice}\n"));
    serve_well_known(&server, "alice.example.com", file).await;
    let dns = ScriptedLookup::default().with(&fqdn("alice.example.com"), resolves_to(PUBLIC));
    let (resolver, _) = resolver_over(&server, dns);

    let did = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert_eq!(did.ok(), Some(alice));
}

#[tokio::test]
async fn when_txt_and_https_disagree_txt_wins_and_https_is_never_asked() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let mallory = plc('m');
    serve_plc_document(&server, &alice, &["alice.example.com"]).await;
    serve_plc_document(&server, &mallory, &["alice.example.com"]).await;
    let file = ResponseTemplate::new(200).set_body_string(mallory.to_string());
    serve_well_known(&server, "alice.example.com", file).await;
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("alice.example.com"), did_record(&alice))
        .with(&fqdn("alice.example.com"), resolves_to(PUBLIC));
    let (resolver, _) = resolver_over(&server, dns);

    let did = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert_eq!(did.ok(), Some(alice));
    assert_eq!(requests_to(&server, "alice.example.com").await, 0);
}

#[tokio::test]
async fn two_txt_records_naming_different_dids_are_not_found() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["alice.example.com"]).await;
    let file = ResponseTemplate::new(200).set_body_string(alice.to_string());
    serve_well_known(&server, "alice.example.com", file).await;
    let conflicting = records(&[
        &format!("did={alice}"),
        &format!("did={}", plc('b')),
        "v=spf1 -all",
    ]);
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("alice.example.com"), conflicting)
        .with(&fqdn("alice.example.com"), resolves_to(PUBLIC));
    let (resolver, _) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
    assert_eq!(requests(&server).await, 0);
}

#[tokio::test]
async fn values_other_than_did_records_are_ignored() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["alice.example.com"]).await;
    let mixed = records(&[
        "v=spf1 -all",
        &format!("did={alice}"),
        "google-site-verification=x",
    ]);
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), mixed);
    let (resolver, _) = resolver_over(&server, dns);

    let did = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert_eq!(did.ok(), Some(alice));
}

#[tokio::test]
async fn a_dns_failure_is_unavailable_and_https_is_never_asked() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let file = ResponseTemplate::new(200).set_body_string(alice.to_string());
    serve_well_known(&server, "alice.example.com", file).await;
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("alice.example.com"), TxtScript::Fail)
        .with(&fqdn("alice.example.com"), resolves_to(PUBLIC));
    let (resolver, _) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::Unavailable(_))),
        "{result:?}"
    );
    assert_eq!(requests(&server).await, 0);
}

#[tokio::test]
async fn a_slow_dns_lookup_is_unavailable_at_the_dns_limit() {
    let server = MockServer::start().await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), TxtScript::Hang);
    let deadlines = Deadlines {
        port_call: Duration::from_secs(30),
        dns: Duration::from_millis(200),
    };
    let (resolver, _) = resolver_with(&server, dns, deadlines);

    let started = std::time::Instant::now();
    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::Unavailable(_))),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the DNS limit ended it, not the port-call deadline"
    );
}

#[tokio::test]
async fn a_handle_too_long_for_dns_is_resolved_over_https() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let long_handle = format!(
        "{}.{}.{}.{}.com",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(55)
    );
    assert_eq!(long_handle.len(), 251);
    serve_plc_document(&server, &alice, &[&long_handle]).await;
    let file = ResponseTemplate::new(200).set_body_string(alice.to_string());
    serve_well_known(&server, &long_handle, file).await;
    let dns = ScriptedLookup::default().with(&fqdn(&long_handle), resolves_to(PUBLIC));
    let (resolver, dns) = resolver_over(&server, dns);

    let did = resolver.resolve_handle(&handle(&long_handle)).await;

    assert_eq!(did.ok(), Some(alice));
    let asked_txt = dns.asked().iter().any(|name| name.starts_with("_atproto."));
    assert!(!asked_txt, "no TXT name was asked: {:?}", dns.asked());
}

#[tokio::test]
async fn a_handle_under_a_reserved_tld_is_not_found_and_nothing_is_asked() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("alice.test"), did_record(&alice))
        .with(&fqdn("alice.test"), resolves_to(PUBLIC));
    let (resolver, dns) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.test")).await;

    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
    assert!(dns.asked().is_empty(), "asked: {:?}", dns.asked());
    assert_eq!(requests(&server).await, 0);
}

// --- the well-known file ---

/// Resolve `alice.example.com` whose only route is a well-known file answering
/// with `file`.
async fn resolve_through_file(file: ResponseTemplate) -> (Result<Did, ResolveError>, MockServer) {
    let server = MockServer::start().await;
    serve_plc_document(&server, &plc('a'), &["alice.example.com"]).await;
    serve_well_known(&server, "alice.example.com", file).await;
    let dns = ScriptedLookup::default().with(&fqdn("alice.example.com"), resolves_to(PUBLIC));
    let (resolver, _) = resolver_over(&server, dns);
    let result = resolver.resolve_handle(&handle("alice.example.com")).await;
    (result, server)
}

#[tokio::test]
async fn a_well_known_file_that_is_not_a_did_is_not_found() {
    let file = ResponseTemplate::new(200).set_body_string("<html>parked domain</html>");
    let (result, _) = resolve_through_file(file).await;
    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_missing_well_known_file_is_not_found() {
    let (result, _) = resolve_through_file(ResponseTemplate::new(404)).await;
    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_well_known_server_error_is_unavailable() {
    let (result, _) = resolve_through_file(ResponseTemplate::new(500)).await;
    assert!(
        matches!(result, Err(ResolveError::Unavailable(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_redirecting_well_known_file_is_refused_and_not_followed() {
    let file = ResponseTemplate::new(302).insert_header(
        "location",
        "https://elsewhere.example.com/.well-known/atproto-did",
    );
    let (result, server) = resolve_through_file(file).await;
    assert!(
        matches!(result, Err(ResolveError::Refused(_))),
        "{result:?}"
    );
    assert_eq!(requests_to(&server, "elsewhere.example.com").await, 0);
}

#[tokio::test]
async fn a_handle_host_at_a_private_address_is_refused() {
    let server = MockServer::start().await;
    let dns = ScriptedLookup::default().with(&fqdn("alice.example.com"), resolves_to(PRIVATE));
    let (resolver, _) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::Refused(_))),
        "{result:?}"
    );
    assert_eq!(requests(&server).await, 0);
}

// --- the back-check ---

#[tokio::test]
async fn a_document_that_does_not_claim_the_handle_is_not_confirmed() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["someone-else.example.com"]).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), did_record(&alice));
    let (resolver, _) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::NotConfirmed)),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_claim_in_the_second_at_uri_only_is_not_confirmed() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["bob.example.com", "alice.example.com"]).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), did_record(&alice));
    let (resolver, _) = resolver_over(&server, dns);

    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::NotConfirmed)),
        "{result:?}"
    );
}

// --- DID → document ---

#[tokio::test]
async fn a_document_naming_another_did_is_not_found() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let body = document(&plc('b'), &["alice.example.com"]);
    serve_plc(
        &server,
        &alice,
        ResponseTemplate::new(200).set_body_json(body),
    )
    .await;
    let (resolver, _) = resolver_over(&server, ScriptedLookup::default());

    let result = resolver.resolve_did(&alice).await;

    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_mini_doc_is_not_found() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let mini_doc = json!({
        "did": alice.as_ref(),
        "handle": "alice.example.com",
        "pds": "https://pds.example.com",
        "signing_key": "zQ3shdiQ4srzeSN8V49TzxWCbNsvj2ZBWaFAB2AS2ii3vTiYK",
    });
    serve_plc(
        &server,
        &alice,
        ResponseTemplate::new(200).set_body_json(mini_doc),
    )
    .await;
    let (resolver, _) = resolver_over(&server, ScriptedLookup::default());

    let result = resolver.resolve_did(&alice).await;

    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_did_plc_the_directory_does_not_have_or_has_tombstoned_is_not_found() {
    let server = MockServer::start().await;
    let unknown = plc('u');
    let tombstoned = plc('t');
    serve_plc(&server, &unknown, ResponseTemplate::new(404)).await;
    serve_plc(&server, &tombstoned, ResponseTemplate::new(410)).await;
    let (resolver, _) = resolver_over(&server, ScriptedLookup::default());

    let unknown = resolver.resolve_did(&unknown).await;
    let tombstoned = resolver.resolve_did(&tombstoned).await;

    assert!(
        matches!(unknown, Err(ResolveError::NotFound)),
        "{unknown:?}"
    );
    assert!(
        matches!(tombstoned, Err(ResolveError::NotFound)),
        "{tombstoned:?}"
    );
}

#[tokio::test]
async fn a_did_outside_the_two_shapes_is_not_found_without_any_request() {
    let server = MockServer::start().await;
    let dns = ScriptedLookup::default()
        .with("alice.example.com.", resolves_to(PUBLIC))
        .with("localhost.", resolves_to(PUBLIC));
    let (resolver, dns) = resolver_over(&server, dns);

    for text in [
        "did:web:alice.example.com%3A8443",
        "did:web:alice.example.com:users:alice",
        "did:web:alice.example.com:443",
        "did:web:localhost",
        "did:web:127.0.0.1",
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        "did:plc:short",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaa1",
    ] {
        let did = Did::from(text.to_string());
        let result = resolver.resolve_did(&did).await;
        assert!(
            matches!(result, Err(ResolveError::NotFound)),
            "{text}: {result:?}"
        );
    }
    assert_eq!(requests(&server).await, 0);
    assert!(dns.asked().is_empty(), "asked: {:?}", dns.asked());
}

#[tokio::test]
async fn a_did_web_host_at_a_private_address_is_refused() {
    let server = MockServer::start().await;
    let dns = ScriptedLookup::default().with("alice.example.com.", resolves_to(PRIVATE));
    let (resolver, _) = resolver_over(&server, dns);
    let did = Did::from("did:web:alice.example.com".to_string());

    let result = resolver.resolve_did(&did).await;

    assert!(
        matches!(result, Err(ResolveError::Refused(_))),
        "{result:?}"
    );
    assert_eq!(requests(&server).await, 0);
}

#[tokio::test]
async fn a_did_web_document_is_read_from_its_host() {
    let server = MockServer::start().await;
    let did = Did::from("did:web:alice.example.com".to_string());
    let body = document(&did, &["alice.example.com"]);
    serve(
        &server,
        "alice.example.com",
        "/.well-known/did.json",
        ResponseTemplate::new(200).set_body_json(body),
    )
    .await;
    let dns = ScriptedLookup::default()
        .with("alice.example.com.", resolves_to(PUBLIC))
        .with_txt(&txt_at("alice.example.com"), did_record(&did));
    let (resolver, _) = resolver_over(&server, dns);

    let handle_of = resolver.resolve_did(&did).await;

    let expected = Some(handle("alice.example.com"));
    assert_eq!(handle_of.ok(), Some(expected));
}

#[tokio::test]
async fn a_plc_directory_timeout_is_unavailable() {
    let server = MockServer::start().await;
    let alice = plc('a');
    let slow = ResponseTemplate::new(200)
        .set_body_json(document(&alice, &[]))
        .set_delay(Duration::from_secs(3));
    serve_plc(&server, &alice, slow).await;
    let (resolver, _) = resolver_over(&server, ScriptedLookup::default());

    let result = resolver.resolve_did(&alice).await;

    assert!(
        matches!(result, Err(ResolveError::Unavailable(_))),
        "{result:?}"
    );
}

// --- DID → handle ---

#[tokio::test]
async fn resolve_did_answers_by_the_back_check() {
    let server = MockServer::start().await;
    let confirmed = plc('a');
    let disowned = plc('b');
    let refused_claim = plc('c');
    let unavailable_claim = plc('d');
    let handleless = plc('e');
    serve_plc_document(&server, &confirmed, &["alice.example.com"]).await;
    serve_plc_document(&server, &disowned, &["victim.example.com"]).await;
    serve_plc_document(&server, &refused_claim, &["refused.example.com"]).await;
    serve_plc_document(&server, &unavailable_claim, &["down.example.com"]).await;
    serve_plc_document(&server, &handleless, &[]).await;
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("alice.example.com"), did_record(&confirmed))
        .with_txt(&txt_at("victim.example.com"), did_record(&plc('v')))
        .with("refused.example.com.", resolves_to(PRIVATE))
        .with_txt(&txt_at("down.example.com"), TxtScript::Fail);
    let (resolver, _) = resolver_over(&server, dns);

    let confirmed = resolver.resolve_did(&confirmed).await;
    let disowned = resolver.resolve_did(&disowned).await;
    let refused_claim = resolver.resolve_did(&refused_claim).await;
    let unavailable_claim = resolver.resolve_did(&unavailable_claim).await;
    let handleless = resolver.resolve_did(&handleless).await;

    let expected_handle = Some(handle("alice.example.com"));
    assert_eq!(confirmed.ok(), Some(expected_handle));
    assert!(matches!(disowned, Ok(None)), "{disowned:?}");
    assert!(matches!(refused_claim, Ok(None)), "{refused_claim:?}");
    assert!(
        matches!(unavailable_claim, Err(ResolveError::Unavailable(_))),
        "{unavailable_claim:?}"
    );
    assert!(matches!(handleless, Ok(None)), "{handleless:?}");
}

#[tokio::test]
async fn a_claim_under_a_disallowed_tld_is_none_and_never_looked_up() {
    let server = MockServer::start().await;
    let alice = plc('a');
    serve_plc_document(&server, &alice, &["alice.local", "alice.example.com"]).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), did_record(&alice));
    let (resolver, dns) = resolver_over(&server, dns);

    let result = resolver.resolve_did(&alice).await;

    assert!(matches!(result, Ok(None)), "{result:?}");
    assert!(
        dns.asked().iter().all(|name| name == "plc.directory."),
        "asked: {:?}",
        dns.asked()
    );
}

// --- the deadline ---

#[tokio::test]
async fn a_call_past_its_deadline_is_unavailable() {
    let server = MockServer::start().await;
    let dns = ScriptedLookup::default().with_txt(&txt_at("alice.example.com"), TxtScript::Hang);
    let deadlines = Deadlines {
        port_call: Duration::from_millis(100),
        dns: Duration::from_secs(30),
    };
    let (resolver, _) = resolver_with(&server, dns, deadlines);

    let started = std::time::Instant::now();
    let result = resolver.resolve_handle(&handle("alice.example.com")).await;

    assert!(
        matches!(result, Err(ResolveError::Unavailable(_))),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the deadline cut it short"
    );
}

// --- the port conformance suite ---

#[tokio::test]
async fn the_real_resolver_passes_the_port_conformance_suite() {
    let server = MockServer::start().await;
    Mock::given(header("host", BLUESKY_APPVIEW))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let confirmed_did = plc('a');
    let unconfirmed_target = plc('b');
    let disowned_did = plc('c');
    let did_claiming_unknown_handle = plc('e');
    let unknown_did = plc('f');
    let unavailable_did = plc('g');
    let did_claiming_unavailable_handle = plc('h');
    let did_claiming_refused_handle = plc('i');
    let handleless_did = Did::from("did:web:handleless.example.com".to_string());
    let refused_did = Did::from("did:web:refused-web.example.com".to_string());

    serve_plc_document(&server, &confirmed_did, &["confirmed.example.com"]).await;
    serve_plc_document(&server, &unconfirmed_target, &["someone-else.example.com"]).await;
    serve_plc_document(&server, &disowned_did, &["disowner.example.com"]).await;
    serve_plc_document(
        &server,
        &did_claiming_unknown_handle,
        &["nobody.example.com"],
    )
    .await;
    serve_plc(&server, &unknown_did, ResponseTemplate::new(404)).await;
    serve_plc(&server, &unavailable_did, ResponseTemplate::new(503)).await;
    serve_plc_document(
        &server,
        &did_claiming_unavailable_handle,
        &["down.example.com"],
    )
    .await;
    serve_plc_document(
        &server,
        &did_claiming_refused_handle,
        &["refused.example.com"],
    )
    .await;
    serve(
        &server,
        "handleless.example.com",
        "/.well-known/did.json",
        ResponseTemplate::new(200).set_body_json(document(&handleless_did, &[])),
    )
    .await;

    let dns = ScriptedLookup::default()
        .with_txt(&txt_at("confirmed.example.com"), did_record(&confirmed_did))
        .with_txt(
            &txt_at("unconfirmed.example.com"),
            did_record(&unconfirmed_target),
        )
        .with_txt(&txt_at("disowner.example.com"), did_record(&plc('d')))
        .with_txt(&txt_at("down.example.com"), TxtScript::Fail)
        .with("refused.example.com.", resolves_to(PRIVATE))
        .with_txt(&txt_at("flaky.example.com"), did_record(&unavailable_did))
        .with("handleless.example.com.", resolves_to(PUBLIC))
        .with("refused-web.example.com.", resolves_to(PRIVATE))
        // A public answer, so a fallback would reach the mock rather than die at DNS.
        .with(&fqdn(BLUESKY_APPVIEW), resolves_to(PUBLIC))
        .with_txt(
            &txt_at("claims-nothing.example.com"),
            did_record(&handleless_did),
        )
        .with_txt(&txt_at("refused-doc.example.com"), did_record(&refused_did))
        .with_txt(&txt_at("missing-doc.example.com"), did_record(&unknown_did));
    let (resolver, dns) = resolver_over(&server, dns);

    let staged = StagedIdentities {
        confirmed_handle: handle("confirmed.example.com"),
        confirmed_did,
        unconfirmed_handle: handle("unconfirmed.example.com"),
        disowned_did,
        handleless_did,
        did_claiming_unknown_handle,
        unknown_handle: handle("unknown.example.com"),
        unknown_did,
        unavailable_handle: handle("down.example.com"),
        refused_handle: handle("refused.example.com"),
        unavailable_did,
        refused_did,
        handle_naming_unavailable_document: handle("flaky.example.com"),
        did_claiming_unavailable_handle,
        did_claiming_refused_handle,
        handle_whose_did_claims_nothing: handle("claims-nothing.example.com"),
        handle_naming_refused_document: handle("refused-doc.example.com"),
        handle_naming_missing_document: handle("missing-doc.example.com"),
    };
    identity_resolver_contract(&resolver, &staged).await;

    let appview_name = fqdn(BLUESKY_APPVIEW);
    assert!(
        !dns.asked().contains(&appview_name),
        "the AppView was looked up: {:?}",
        dns.asked()
    );
    assert_eq!(requests_to(&server, BLUESKY_APPVIEW).await, 0);
}

// --- what the logs carry ---

#[tokio::test]
async fn info_logs_only_the_failure_class_and_debug_carries_the_cause() {
    let server = MockServer::start().await;
    let unavailable_file = ResponseTemplate::new(503);
    serve_well_known(&server, "outage.example.com", unavailable_file).await;
    let dns = ScriptedLookup::default()
        .with(&fqdn("outage.example.com"), resolves_to(PUBLIC))
        .with(&fqdn("private.example.com"), resolves_to(PRIVATE));
    let (resolver, _) = resolver_over(&server, dns);

    let ((outage, refused), events) = record_events(async {
        let outage = resolver.resolve_handle(&handle("outage.example.com")).await;
        let refused = resolver
            .resolve_handle(&handle("private.example.com"))
            .await;
        (outage, refused)
    })
    .await;

    assert!(
        matches!(outage, Err(ResolveError::Unavailable(_))),
        "{outage:?}"
    );
    assert!(
        matches!(refused, Err(ResolveError::Refused(_))),
        "{refused:?}"
    );
    let info = lines_at(&events, tracing::Level::INFO, false);
    let expected_info = vec![
        "message=identity lookup failed failure=Unavailable ".to_string(),
        "message=identity lookup failed failure=Refused ".to_string(),
    ];
    assert_eq!(info, expected_info);
    for line in lines_at(&events, tracing::Level::INFO, true) {
        for secret in [
            "outage.example.com",
            "private.example.com",
            "503",
            "forbidden address",
        ] {
            assert!(
                !line.contains(secret),
                "an info line carries {secret:?}: {line}"
            );
        }
    }
    let debug = lines_at(&events, tracing::Level::DEBUG, false).join("\n");
    assert!(
        debug.contains("the host answered 503"),
        "the cause is at debug: {debug}"
    );
    assert!(
        debug.contains("forbidden address"),
        "the cause is at debug: {debug}"
    );
}
