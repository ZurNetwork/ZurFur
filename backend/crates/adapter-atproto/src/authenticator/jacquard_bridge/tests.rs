use std::error::Error as _;

use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::guarded_http::{Script, ScriptedLookup, Timeouts};

const PUBLIC: &str = "93.184.216.34";

fn plc(fill: char) -> AtDid {
    let id: String = std::iter::repeat_n(fill, 24).collect();
    AtDid::new_owned(format!("did:plc:{id}")).expect("a DID")
}

/// A bridge whose PLC directory is `server`.
fn bridge(server: &MockServer) -> JacquardBridge {
    let public = Script::Answer(vec![PUBLIC.parse().expect("a test address")]);
    let dns = Arc::new(ScriptedLookup::default().with("plc.directory.", public));
    let timeouts = Timeouts {
        fetch: Duration::from_secs(2),
        dns: Duration::from_millis(500),
    };
    let http = GuardedHttp::upstream(*server.address(), dns.clone(), timeouts);
    let resolver = Arc::new(AtprotoIdentityResolver::scripted(http, dns));
    JacquardBridge::new(resolver)
}

async fn serve_plc(server: &MockServer, did: &AtDid, body: String) {
    Mock::given(method("GET"))
        .and(header("host", "plc.directory"))
        .and(path(format!("/{}", did.as_str())))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn resolve_handle_always_fails_without_a_request() {
    let server = MockServer::start().await;
    let bridge = bridge(&server);
    let handle: Handle = Handle::new_owned("alice.bsky.social").expect("a handle");

    let result = bridge.resolve_handle(&handle).await;

    let error = result.expect_err("the bridge never resolves a handle");
    let tripped = error
        .source()
        .is_some_and(|cause| cause.is::<HandleTripwire>());
    assert!(tripped, "the failure is the tripwire");
    let requests = server.received_requests().await.unwrap_or_default();
    assert!(requests.is_empty(), "no request was made");
}

#[tokio::test]
async fn resolve_did_doc_hands_jacquard_the_checked_body() {
    let server = MockServer::start().await;
    let did = plc('a');
    let body = json!({
        "id": did.as_str(),
        "alsoKnownAs": ["at://alice.example.com"],
        "service": [],
    })
    .to_string();
    serve_plc(&server, &did, body.clone()).await;

    let response = bridge(&server)
        .resolve_did_doc(&did)
        .await
        .expect("a checked document");

    assert_eq!(
        response.buffer,
        body.as_bytes(),
        "the bytes that passed the checks"
    );
    assert_eq!(response.status, http::StatusCode::OK);
    assert_eq!(
        response.requested.as_ref().map(AtDid::as_str),
        Some(did.as_str())
    );
}

#[tokio::test]
async fn resolve_did_doc_refuses_a_mini_doc_and_a_document_for_another_did() {
    let server = MockServer::start().await;
    let mini = plc('m');
    let mini_doc = json!({
        "did": mini.as_str(),
        "handle": "mini.example.com",
        "signingKey": "did:key:zQ3shunBKsXixLxKtC5qeSG9E4J5RkGN57im31pcTzbNQnm5w",
        "pds": "https://pds.example.com",
    });
    serve_plc(&server, &mini, mini_doc.to_string()).await;
    let impostor = plc('i');
    let someone_else = json!({ "id": plc('z').as_str() });
    serve_plc(&server, &impostor, someone_else.to_string()).await;
    let bridge = bridge(&server);

    for did in [mini, impostor] {
        let result = bridge.resolve_did_doc(&did).await;
        assert!(result.is_err(), "{} must not pass", did.as_str());
    }
}

#[tokio::test]
async fn resolve_did_doc_never_fetches_a_did_web_with_a_path() {
    let server = MockServer::start().await;
    let with_path: AtDid = AtDid::new_owned("did:web:example.com:users:alice").expect("a DID");

    let result = bridge(&server).resolve_did_doc(&with_path).await;

    assert!(result.is_err());
    let requests = server.received_requests().await.unwrap_or_default();
    assert!(requests.is_empty(), "no request was made");
}

#[test]
fn options_turn_every_fallback_off() {
    let options = no_fallbacks();
    assert!(options.handle_order.is_empty(), "no handle steps");
    assert!(options.did_order.is_empty(), "no document steps");
    assert!(options.pds_fallback.is_none(), "no PDS fallback");
    assert!(!options.public_fallback_for_handle, "no Bluesky fallback");
}

#[tokio::test]
async fn the_issuer_check_document_fetch_gives_up_at_its_deadline() {
    let server = MockServer::start().await;
    let did = plc('s');
    let late = ResponseTemplate::new(200)
        .set_body_string(json!({ "id": did.as_str() }).to_string())
        .set_delay(Duration::from_secs(5));
    Mock::given(method("GET"))
        .and(header("host", "plc.directory"))
        .and(path(format!("/{}", did.as_str())))
        .respond_with(late)
        .mount(&server)
        .await;
    let bridge = bridge(&server).with_document_deadline(Duration::from_millis(300));

    let begun = std::time::Instant::now();
    let result = bridge.resolve_did_doc(&did).await;

    assert!(
        result.is_err(),
        "a document past the deadline is no document"
    );
    // Well under the 2 s fetch timeout, so only the deadline explains it.
    assert!(
        begun.elapsed() < Duration::from_secs(1),
        "the deadline cut it off"
    );
}
