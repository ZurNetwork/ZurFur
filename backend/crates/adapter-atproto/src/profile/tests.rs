use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::guarded_http::{GuardedHttp, Script, ScriptedLookup, Timeouts, TxtScript};
use crate::resolver::log_recorder::{lines_at, record_events};

const HANDLE: &str = "alice.example.com";
const PDS_HOST: &str = "pds.example.com";
const PDS: &str = "https://pds.example.com";
/// Every staged host answers with this public address; the upstream route then
/// delivers the request to the mock.
const PUBLIC: &str = "93.184.216.34";
const RECORD_PATH: &str = "/xrpc/com.atproto.repo.getRecord";

fn plc(fill: char) -> Did {
    let id: String = std::iter::repeat_n(fill, 24).collect();
    Did::from(format!("did:plc:{id}"))
}

fn public_address() -> Script {
    let address = PUBLIC.parse().expect("a test address");
    Script::Answer(vec![address])
}

/// A TXT answer naming `did` for `_atproto.<handle>.`.
fn did_record(did: &Did) -> TxtScript {
    TxtScript::Records(vec![format!("did={did}")])
}

fn txt_at(handle: &str) -> String {
    format!("_atproto.{handle}.")
}

/// A DID document for `did` claiming `also_known_as`, its PDS at `pds`.
fn document(did: &Did, also_known_as: &[&str], pds: &str) -> serde_json::Value {
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
            "serviceEndpoint": pds,
        }],
    })
}

/// Serve `did`'s document from the PLC directory with `response`.
async fn serve_plc(server: &MockServer, did: &Did, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(header("host", "plc.directory"))
        .and(path(format!("/{did}")))
        .respond_with(response)
        .mount(server)
        .await;
}

/// Serve the document `did` claiming `also_known_as`, its PDS at `pds`.
async fn serve_document(server: &MockServer, did: &Did, also_known_as: &[&str], pds: &str) {
    let body = document(did, also_known_as, pds);
    serve_plc(server, did, ResponseTemplate::new(200).set_body_json(body)).await;
}

/// Answer the PDS's `getRecord` for `did`'s profile record with `response`.
async fn serve_record(server: &MockServer, did: &Did, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(header("host", PDS_HOST))
        .and(path(RECORD_PATH))
        .and(query_param("repo", did.as_ref()))
        .and(query_param("collection", "app.bsky.actor.profile"))
        .and(query_param("rkey", "self"))
        .respond_with(response)
        .mount(server)
        .await;
}

/// How many `getRecord` calls reached `server`, whatever their host.
async fn record_reads(server: &MockServer) -> usize {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| request.url.path() == RECORD_PATH)
        .count()
}

/// The source over `dns` (plus the PLC directory and the PDS) and the
/// upstream route to `server`.
fn source_over(server: &MockServer, dns: ScriptedLookup) -> AtprotoProfileSource {
    let resolver = resolver_over(server, dns);
    AtprotoProfileSource::new(Arc::new(resolver))
}

/// The resolver behind [`source_over`], before it is shared.
fn resolver_over(server: &MockServer, dns: ScriptedLookup) -> AtprotoIdentityResolver {
    let dns = Arc::new(
        dns.with("plc.directory.", public_address())
            .with(&format!("{PDS_HOST}."), public_address()),
    );
    let timeouts = Timeouts {
        fetch: Duration::from_secs(1),
        dns: Duration::from_millis(200),
    };
    let http = GuardedHttp::upstream(*server.address(), dns.clone(), timeouts);
    AtprotoIdentityResolver::scripted(http, dns)
}

/// How many requests carrying `Host: host` reached `server`.
async fn requests_to(server: &MockServer, host: &str) -> usize {
    let requests = server.received_requests().await.unwrap_or_default();
    requests
        .iter()
        .filter(|request| {
            let request_host = request.headers.get("host");
            request_host.and_then(|value| value.to_str().ok()) == Some(host)
        })
        .count()
}

/// `did`'s document claims [`HANDLE`] at [`PDS`], and the handle names `did` back.
async fn confirmed_source(server: &MockServer, did: &Did) -> AtprotoProfileSource {
    serve_document(server, did, &[HANDLE], PDS).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at(HANDLE), did_record(did));
    source_over(server, dns)
}

fn profile_record(did: &Did, value: serde_json::Value) -> ResponseTemplate {
    let record = json!({
        "uri": format!("at://{did}/app.bsky.actor.profile/self"),
        "cid": "bafyreicidoftherecorditself",
        "value": value,
    });
    ResponseTemplate::new(200).set_body_json(record)
}

#[tokio::test]
async fn a_full_profile_is_read_from_the_validated_pds() {
    let server = MockServer::start().await;
    let did = plc('a');
    let source = confirmed_source(&server, &did).await;
    let value = json!({
        "$type": "app.bsky.actor.profile",
        "displayName": "Alice",
        "avatar": {
            "$type": "blob",
            "ref": { "$link": "bafkreicidoftheavatarblob" },
            "mimeType": "image/png",
            "size": 12345,
        },
    });
    serve_record(&server, &did, profile_record(&did, value)).await;

    let profile = source.fetch(&did).await.expect("the read succeeds");

    let expected_avatar = format!(
        "https://pds.example.com/xrpc/com.atproto.sync.getBlob?did=did%3Aplc%3A{}&cid=bafkreicidoftheavatarblob",
        "a".repeat(24)
    );
    assert_eq!(profile.handle.to_string(), HANDLE);
    assert_eq!(profile.display_name.as_deref(), Some("Alice"));
    assert_eq!(profile.avatar_url, Some(expected_avatar));
    // One document fetch serves both the PDS and the handle's back-check.
    assert_eq!(requests_to(&server, "plc.directory").await, 1);
}

#[tokio::test]
async fn a_missing_profile_record_yields_a_handle_only_profile() {
    let server = MockServer::start().await;
    let did = plc('a');
    let source = confirmed_source(&server, &did).await;
    let not_found = json!({ "error": "RecordNotFound", "message": "could not locate record" });
    serve_record(
        &server,
        &did,
        ResponseTemplate::new(400).set_body_json(not_found),
    )
    .await;

    let profile = source
        .fetch(&did)
        .await
        .expect("a missing record is not an error");

    assert_eq!(profile.handle.to_string(), HANDLE);
    assert_eq!(profile.display_name, None);
    assert_eq!(profile.avatar_url, None);
}

#[tokio::test]
async fn a_claimed_handle_naming_another_did_is_shown_as_the_did() {
    let server = MockServer::start().await;
    let did = plc('a');
    serve_document(&server, &did, &[HANDLE], PDS).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at(HANDLE), did_record(&plc('z')));
    let source = source_over(&server, dns);
    let empty = json!({ "$type": "app.bsky.actor.profile" });
    serve_record(&server, &did, profile_record(&did, empty)).await;

    let profile = source.fetch(&did).await.expect("the read succeeds");

    assert_eq!(profile.handle.to_string(), did.to_string());
}

#[tokio::test]
async fn a_document_claiming_no_handle_is_shown_as_the_did() {
    let server = MockServer::start().await;
    let did = plc('a');
    serve_document(&server, &did, &[], PDS).await;
    let source = source_over(&server, ScriptedLookup::default());
    let empty = json!({ "$type": "app.bsky.actor.profile" });
    serve_record(&server, &did, profile_record(&did, empty)).await;

    let profile = source.fetch(&did).await.expect("the read succeeds");

    assert_eq!(profile.handle.to_string(), did.to_string());
}

#[tokio::test]
async fn a_failing_pds_is_an_error() {
    let server = MockServer::start().await;
    let did = plc('a');
    let source = confirmed_source(&server, &did).await;
    serve_record(&server, &did, ResponseTemplate::new(500)).await;

    let result = source.fetch(&did).await;

    assert!(result.is_err(), "got: {result:?}");
}

#[tokio::test]
async fn a_record_error_other_than_not_found_is_an_error_never_a_stripped_profile() {
    let did = plc('a');
    let invalid_request = json!({ "error": "InvalidRequest", "message": "bad repo" });
    let answers = [
        (
            "a 400 InvalidRequest",
            ResponseTemplate::new(400).set_body_json(invalid_request),
        ),
        (
            "a 200 that is no record",
            ResponseTemplate::new(200).set_body_string("not a record"),
        ),
        (
            "a 200 record of the wrong shape",
            ResponseTemplate::new(200).set_body_json(json!({ "value": 42 })),
        ),
    ];

    let mut stripped = Vec::new();
    for (answer, response) in answers {
        let server = MockServer::start().await;
        let source = confirmed_source(&server, &did).await;
        serve_record(&server, &did, response).await;

        let result = source.fetch(&did).await;

        if result.is_ok() {
            stripped.push(format!("{answer}: {result:?}"));
        }
    }

    assert_eq!(stripped, Vec::<String>::new());
}

#[tokio::test]
async fn the_whole_read_gives_up_at_the_port_call_deadline() {
    let server = MockServer::start().await;
    let did = plc('a');
    // Each step fits the deadline on its own; together they do not.
    let body = document(&did, &[HANDLE], PDS);
    let slow_document = ResponseTemplate::new(200)
        .set_body_json(body)
        .set_delay(Duration::from_millis(200));
    serve_plc(&server, &did, slow_document).await;
    let empty = json!({ "$type": "app.bsky.actor.profile" });
    let slow_record = profile_record(&did, empty).set_delay(Duration::from_millis(250));
    serve_record(&server, &did, slow_record).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at(HANDLE), did_record(&did));
    let resolver = resolver_over(&server, dns).with_port_call_deadline(Duration::from_millis(300));
    let source = AtprotoProfileSource::new(Arc::new(resolver));
    let started = std::time::Instant::now();

    let result = source.fetch(&did).await;

    let elapsed = started.elapsed();
    assert!(result.is_err(), "got: {result:?}");
    assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
}

#[tokio::test]
async fn a_pds_the_url_policy_refuses_is_an_error_and_never_asked() {
    let pds_endpoints = [
        "http://pds.example.com",
        "https://pds.example.com:8443",
        "https://10.0.0.1",
        "https://127.0.0.1",
    ];
    let did = plc('a');

    let mut wrongly_read = Vec::new();
    for endpoint in pds_endpoints {
        let server = MockServer::start().await;
        serve_document(&server, &did, &[HANDLE], endpoint).await;
        let dns = ScriptedLookup::default().with_txt(&txt_at(HANDLE), did_record(&did));
        let source = source_over(&server, dns);
        Mock::given(path(RECORD_PATH))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let result = source.fetch(&did).await;

        let reads = record_reads(&server).await;
        if result.is_ok() || reads > 0 {
            wrongly_read.push(format!("{endpoint}: {result:?}, {reads} reads"));
        }
    }

    assert_eq!(wrongly_read, Vec::<String>::new());
}

#[tokio::test]
async fn an_outage_in_the_handle_back_check_is_an_error_so_nothing_is_cached() {
    let server = MockServer::start().await;
    let did = plc('a');
    serve_document(&server, &did, &[HANDLE], PDS).await;
    let dns = ScriptedLookup::default().with_txt(&txt_at(HANDLE), TxtScript::Fail);
    let source = source_over(&server, dns);
    let empty = json!({ "$type": "app.bsky.actor.profile" });
    serve_record(&server, &did, profile_record(&did, empty)).await;

    let result = source.fetch(&did).await;

    assert!(result.is_err(), "got: {result:?}");
}

#[tokio::test]
async fn an_unavailable_did_document_is_an_error() {
    let server = MockServer::start().await;
    let did = plc('a');
    serve_plc(&server, &did, ResponseTemplate::new(503)).await;
    let source = source_over(&server, ScriptedLookup::default());

    let result = source.fetch(&did).await;

    assert!(result.is_err(), "got: {result:?}");
    assert_eq!(record_reads(&server).await, 0);
}

// --- what a failure writes to the log ---

#[tokio::test]
async fn a_failed_read_logs_its_class_at_info_and_never_the_handle_or_host() {
    let server = MockServer::start().await;
    let outage = plc('a');
    let refused_pds = plc('b');
    let failing_pds = plc('c');
    serve_document(&server, &outage, &[HANDLE], PDS).await;
    serve_document(
        &server,
        &refused_pds,
        &["bob.example.com"],
        "https://10.0.0.1",
    )
    .await;
    serve_document(&server, &failing_pds, &["carol.example.com"], PDS).await;
    let dns = ScriptedLookup::default()
        .with_txt(&txt_at(HANDLE), TxtScript::Fail)
        .with_txt(&txt_at("bob.example.com"), did_record(&refused_pds))
        .with_txt(&txt_at("carol.example.com"), did_record(&failing_pds));
    let source = source_over(&server, dns);
    serve_record(&server, &failing_pds, ResponseTemplate::new(500)).await;

    let (_, events) = record_events(async {
        let _ = source.fetch(&outage).await;
        let _ = source.fetch(&refused_pds).await;
        let _ = source.fetch(&failing_pds).await;
    })
    .await;

    let info_and_above = lines_at(&events, tracing::Level::INFO, true);
    let profile_lines: Vec<&str> = info_and_above
        .iter()
        .map(String::as_str)
        .filter(|line| line.contains("profile read failed"))
        .collect();
    // Exactly the message and the class: no handle, host or cause field.
    let expected = [
        "message=profile read failed failure=\"identity_unavailable\" ",
        "message=profile read failed failure=\"pds_unusable\" ",
        "message=profile read failed failure=\"record_unreadable\" ",
    ];
    assert_eq!(profile_lines, expected);
    let info_and_above = info_and_above.join("\n");
    let leaked: Vec<&str> = [
        HANDLE,
        "bob.example.com",
        "carol.example.com",
        PDS_HOST,
        "10.0.0.1",
        "plc.directory",
    ]
    .into_iter()
    .filter(|secret| info_and_above.contains(secret))
    .collect();
    assert_eq!(leaked, Vec::<&str>::new(), "log: {info_and_above}");
}

// --- the avatar URL ---

fn pds_at(text: &str) -> PublicHttpsUrl {
    PublicHttpsUrl::try_from(text).expect("a PDS the policy admits")
}

#[test]
fn the_avatar_url_keeps_the_pds_path_prefix_and_drops_its_query_and_fragment() {
    let did = plc('a');
    let pds_addresses = [
        (
            "https://pds.example.com",
            "https://pds.example.com/xrpc/com.atproto.sync.getBlob",
        ),
        (
            "https://pds.example.com/#x",
            "https://pds.example.com/xrpc/com.atproto.sync.getBlob",
        ),
        (
            "https://pds.example.com/?q=1",
            "https://pds.example.com/xrpc/com.atproto.sync.getBlob",
        ),
        (
            "https://pds.example.com/base",
            "https://pds.example.com/base/xrpc/com.atproto.sync.getBlob",
        ),
        (
            "https://pds.example.com/base/",
            "https://pds.example.com/base/xrpc/com.atproto.sync.getBlob",
        ),
    ];

    let wrong: Vec<String> = pds_addresses
        .iter()
        .filter_map(|(pds, expected_endpoint)| {
            let avatar = blob_url(&pds_at(pds), &did, "bafkreicid");
            let expected = format!(
                "{expected_endpoint}?did=did%3Aplc%3A{}&cid=bafkreicid",
                "a".repeat(24)
            );
            (avatar != expected).then(|| format!("{pds}: {avatar}"))
        })
        .collect();

    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn a_hostile_avatar_cid_stays_one_encoded_query_value() {
    let did = plc('a');
    let hostile_cid = "x&did=did:plc:someoneelse#fragment";

    let avatar = blob_url(&pds_at(PDS), &did, hostile_cid);

    let parsed = url::Url::parse(&avatar).expect("the avatar URL parses");
    let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
    let expected_pairs = vec![
        ("did".to_string(), did.to_string()),
        ("cid".to_string(), hostile_cid.to_string()),
    ];
    assert_eq!(pairs, expected_pairs);
    assert_eq!(parsed.fragment(), None);
    assert_eq!(parsed.host_str(), Some(PDS_HOST));
}
