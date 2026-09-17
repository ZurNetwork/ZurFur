use super::{AtprotoProfileSource, presented_handle};
use domain::elements::did::Did as DomainDid;
use domain::ports::ProfileSource as _;
use jacquard::common::deps::fluent_uri::Uri;
use jacquard::identity::resolver::{DidStep, HandleStep, PlcSource, ResolverOptions};
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "did:plc:actor";

// Finding 2: a handle is only the actor's when it resolves BACK to this DID.
#[test]
fn a_handle_that_resolves_back_to_this_did_is_presented() {
    assert_eq!(
        presented_handle(DID, "alice.zurfur.app", Some(DID)),
        "alice.zurfur.app",
        "a bidirectionally-verified handle is trusted"
    );
}

#[test]
fn a_handle_resolving_to_another_did_is_never_presented() {
    // A spoofed / stale `alsoKnownAs`: the claimed handle belongs to someone else.
    assert_eq!(
        presented_handle(DID, "victim.zurfur.app", Some("did:plc:someoneelse")),
        DID,
        "a handle owned by a different DID falls back to the DID, never impersonates"
    );
}

#[test]
fn an_unresolvable_handle_falls_back_to_the_did() {
    // Reverse resolution could not be completed (malformed handle, resolver
    // failure, …) — the claim is unconfirmed, so it must not be presented.
    assert_eq!(
        presented_handle(DID, "alice.zurfur.app", None),
        DID,
        "an unconfirmable handle is never presented as trusted"
    );
}

// --- fetch() against a mock PDS/PLC-directory server -----------------------

const ACTOR_DID: &str = "did:plc:mockactor1234567";
const ACTOR_HANDLE: &str = "alice.zurfur.app";

/// Point a fresh source at `server` for both the PLC directory and the PDS
/// fallback: PlcHttp only for DID docs, PdsResolveHandle only for handles, no
/// public-API fallback — every resolution hits the mock or fails.
fn source_for(server: &MockServer) -> AtprotoProfileSource {
    let plc_base = Uri::parse(format!("{}/", server.uri()))
        .expect("mock server uri is valid")
        .to_owned();
    let pds_fallback = Uri::parse(server.uri())
        .expect("mock server uri is valid")
        .to_owned();
    let options = ResolverOptions::new()
        .plc_source(PlcSource::PlcDirectory { base: plc_base })
        .pds_fallback(pds_fallback)
        .handle_order(vec![HandleStep::PdsResolveHandle])
        .did_order(vec![DidStep::PlcHttp])
        .validate_doc_id(true)
        .public_fallback_for_handle(false)
        .request_timeout(Duration::from_secs(20))
        .build();
    AtprotoProfileSource::with_resolver_options(options)
}

/// Mount the DID document at `GET /{did}`, claiming `handle` and pointing its
/// PDS service endpoint at `server` itself.
async fn mount_did_doc(server: &MockServer, did: &str, handle: &str) {
    let doc = json!({
        "id": did,
        "alsoKnownAs": [format!("at://{handle}")],
        "verificationMethod": [{
            "id": format!("{did}#atproto"),
            "type": "Multikey",
            "publicKeyMultibase": "zQ3shdiQ4srzeSN8V49TzxWCbNsvj2ZBWaFAB2AS2ii3vTiYK",
        }],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": server.uri(),
        }],
    });
    Mock::given(method("GET"))
        .and(path(format!("/{did}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(doc))
        .mount(server)
        .await;
}

/// Mount `com.atproto.identity.resolveHandle` answering `did` for `handle`.
async fn mount_resolve_handle(server: &MockServer, handle: &str, did: &str) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.identity.resolveHandle"))
        .and(query_param("handle", handle))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "did": did })))
        .mount(server)
        .await;
}

/// Mount `com.atproto.repo.getRecord` for `app.bsky.actor.profile/self` on
/// `did`, answering with `response`.
async fn mount_get_record(server: &MockServer, did: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("repo", did))
        .and(query_param("collection", "app.bsky.actor.profile"))
        .and(query_param("rkey", "self"))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn full_profile_is_fetched_from_the_mock_pds() {
    let server = MockServer::start().await;
    mount_did_doc(&server, ACTOR_DID, ACTOR_HANDLE).await;
    mount_resolve_handle(&server, ACTOR_HANDLE, ACTOR_DID).await;
    let record = json!({
        "uri": format!("at://{ACTOR_DID}/app.bsky.actor.profile/self"),
        "cid": "bafyreicidoftherecorditself",
        "value": {
            "$type": "app.bsky.actor.profile",
            "displayName": "Alice",
            "avatar": {
                "$type": "blob",
                "ref": { "$link": "bafkreicidoftheavatarblob" },
                "mimeType": "image/png",
                "size": 12345,
            },
        },
    });
    mount_get_record(
        &server,
        ACTOR_DID,
        ResponseTemplate::new(200).set_body_json(record),
    )
    .await;

    let source = source_for(&server);
    let did: DomainDid = ACTOR_DID.to_string().into();
    let profile = source.fetch(&did).await.expect("fetch succeeds");

    assert_eq!(profile.handle.to_string(), ACTOR_HANDLE);
    assert_eq!(profile.display_name, Some("Alice".to_string()));
    let avatar_url = profile.avatar_url.expect("avatar url present");
    let expected_prefix = format!(
        "{}/xrpc/com.atproto.sync.getBlob?did={ACTOR_DID}",
        server.uri()
    );
    assert!(
        avatar_url.starts_with(&expected_prefix),
        "got: {avatar_url}"
    );
}

#[tokio::test]
async fn a_missing_profile_record_yields_a_handle_only_profile() {
    let server = MockServer::start().await;
    mount_did_doc(&server, ACTOR_DID, ACTOR_HANDLE).await;
    mount_resolve_handle(&server, ACTOR_HANDLE, ACTOR_DID).await;
    let not_found = json!({ "error": "RecordNotFound", "message": "could not locate record" });
    mount_get_record(
        &server,
        ACTOR_DID,
        ResponseTemplate::new(400).set_body_json(not_found),
    )
    .await;

    let source = source_for(&server);
    let did: DomainDid = ACTOR_DID.to_string().into();
    let profile = source
        .fetch(&did)
        .await
        .expect("a missing record is not an error");

    assert_eq!(profile.display_name, None);
    assert_eq!(profile.avatar_url, None);
}

#[tokio::test]
async fn a_handle_resolving_to_another_did_is_presented_as_the_did() {
    let server = MockServer::start().await;
    mount_did_doc(&server, ACTOR_DID, ACTOR_HANDLE).await;
    // The claimed handle actually resolves back to someone else.
    mount_resolve_handle(&server, ACTOR_HANDLE, "did:plc:someoneelse1234567").await;
    let record = json!({
        "uri": format!("at://{ACTOR_DID}/app.bsky.actor.profile/self"),
        "value": { "$type": "app.bsky.actor.profile" },
    });
    mount_get_record(
        &server,
        ACTOR_DID,
        ResponseTemplate::new(200).set_body_json(record),
    )
    .await;

    let source = source_for(&server);
    let did: DomainDid = ACTOR_DID.to_string().into();
    let profile = source.fetch(&did).await.expect("fetch succeeds");

    assert_eq!(profile.handle.to_string(), ACTOR_DID);
}

#[tokio::test]
async fn a_failing_pds_is_an_error() {
    let server = MockServer::start().await;
    mount_did_doc(&server, ACTOR_DID, ACTOR_HANDLE).await;
    mount_resolve_handle(&server, ACTOR_HANDLE, ACTOR_DID).await;
    mount_get_record(&server, ACTOR_DID, ResponseTemplate::new(500)).await;

    let source = source_for(&server);
    let did: DomainDid = ACTOR_DID.to_string().into();
    let result = source.fetch(&did).await;

    assert!(result.is_err(), "got: {result:?}");
}
