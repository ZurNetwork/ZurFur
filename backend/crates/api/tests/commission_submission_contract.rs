//! ZMVP-89 — the submission contract: file upload and Status-set are **two
//! explicit, never-coupled API calls** (Engineer ruling 2026-07-05: no UI ships
//! in this epic; the future submission form orchestrates these two calls — no
//! backend shortcut, no coupled write exists, by design).
//!
//! Pins the always-explicit rule (DESIGN/Commission — Status; explicit-transition
//! ruling 2026-07-01) at the API surface:
//!
//! - **Negative contract** — `POST /commissions/{id}/files` NEVER mutates any
//!   status: not the direction axis, not the deadline axis, not the Lifecycle —
//!   even when the request smuggles a `status` alongside the upload (a multipart
//!   field and a query parameter), it is ignored, never applied.
//! - **The walkthrough shape** — upload then explicit Status-set as two separate
//!   calls, each landing its **own** changelog entry (`file_added`, then
//!   `status_changed`), exactly as the ZMVP-91 walkthrough will exercise it.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{CommissionId, DeadlineStatus, DirectionStatus},
    did::Did,
    profile::Profile,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] for introspection. `did` is the identity `sign_in` authenticates as.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let backend = MemBackend::new();
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".to_string(),
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "artist.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
}

/// A cookie-keeping client that does not auto-follow redirects.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so the client's cookie jar carries a live session.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=artist.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303);
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
}

/// Creates a commission over HTTP (with the given body) and returns its id
/// (introspected off the backend — the route returns a bare `201`).
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
    body: serde_json::Value,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&body)
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");
    let all = backend.all_commissions().await.expect("list commissions");
    *all.last().expect("a commission was persisted").id
}

/// The commission's changelog entries (introspected off the backend).
async fn entries(
    backend: &MemBackend,
    id: uuid::Uuid,
) -> Vec<domain::elements::commission::ChangelogEntry> {
    backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("inspect entries")
}

/// The persisted commission, freshly read.
async fn stored(backend: &MemBackend, id: uuid::Uuid) -> domain::elements::commission::Commission {
    backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists")
}

// The negative contract (Engineer ruling 2026-07-05): the upload endpoint NEVER
// mutates any status. A commission holding a value on EVERY status axis —
// Lifecycle (draft), direction (waiting_for_input), deadline (Late, via a real
// sweep) — takes an upload that actively tries to smuggle a status along (a
// multipart `status` field before the `file` part, plus a `?status=` query
// parameter). The upload succeeds; every axis is untouched; the only new
// changelog entry is the `file_added` itself, whose payload carries no status.
#[tokio::test]
async fn the_upload_endpoint_never_mutates_any_status() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    // A deadline already in the past, so a sweep can put a real value on the
    // deadline axis.
    let past = Utc::now() - chrono::Duration::hours(2);
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref sheet", "deadline": past.to_rfc3339() }),
    )
    .await;
    let marked = api::sweep_deadlines(&*backend.database(), Utc::now())
        .await
        .expect("sweep runs");
    assert_eq!(marked, 1, "the seeded commission is marked Late");

    // And a value on the direction axis, set explicitly (ZMVP-85 machinery).
    let res = client
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "status": "waiting_for_input" }))
        .send()
        .await
        .expect("PUT direction status");
    assert_eq!(res.status(), 204);

    // Snapshot every axis, and the changelog length, before the upload.
    let before = stored(&backend, id).await;
    assert_eq!(
        before.direction_status,
        Some(DirectionStatus::WaitingForInput)
    );
    assert_eq!(before.deadline_status, Some(DeadlineStatus::Late));
    let log_before = entries(&backend, id).await.len();

    // The upload smuggles a status two ways: a multipart field ahead of the
    // file part, and a query parameter. Neither is part of the contract; both
    // must be ignored — never applied.
    let part = reqwest::multipart::Part::bytes(b"PNG-BYTES".to_vec())
        .file_name("sketch.png")
        .mime_str("image/png")
        .expect("valid mime");
    let form = reqwest::multipart::Form::new()
        .text("status", "waiting_for_approval")
        .part("file", part);
    let res = client
        .post(format!(
            "{base}/commissions/{id}/files?status=waiting_for_approval"
        ))
        .multipart(form)
        .send()
        .await
        .expect("POST file");
    assert_eq!(res.status(), 201, "the upload itself succeeds");

    // Every status axis is exactly as it was.
    let after = stored(&backend, id).await;
    assert_eq!(
        after.lifecycle_step.as_str(),
        before.lifecycle_step.as_str(),
        "an upload never moves the Lifecycle"
    );
    assert_eq!(
        after.direction_status,
        Some(DirectionStatus::WaitingForInput),
        "an upload never moves the direction axis — not even a smuggled status"
    );
    assert_eq!(
        after.deadline_status,
        Some(DeadlineStatus::Late),
        "an upload never moves the deadline axis"
    );
    assert_eq!(
        after.deadline, before.deadline,
        "the deadline itself is untouched"
    );

    // Exactly one new entry — the upload's own — and it carries no status.
    let log = entries(&backend, id).await;
    assert_eq!(
        log.len(),
        log_before + 1,
        "the upload appends exactly its own entry, nothing else"
    );
    let uploaded = log.last().expect("the new entry");
    assert_eq!(uploaded.kind.as_str(), "file_added");
    // Sorted before comparing: serde_json's Map iteration order is
    // feature-dependent (`preserve_order`), and the contract pins the key SET.
    let mut payload_keys: Vec<&str> = uploaded
        .payload
        .as_object()
        .expect("payload is an object")
        .keys()
        .map(String::as_str)
        .collect();
    payload_keys.sort_unstable();
    assert_eq!(
        payload_keys,
        ["byte_size", "content_type", "file_id", "filename"],
        "the file_added payload names the file and nothing else — no status"
    );
    assert!(
        !log.iter()
            .skip(log_before)
            .any(|e| e.kind.as_str() == "status_changed"),
        "no status_changed entry rode along with the upload"
    );
}

// The walkthrough shape (ZMVP-91 will exercise this verbatim): upload, then
// explicit Status-set, as two separate calls — each producing its OWN changelog
// entry, both authored by the acting Participant. This is the API contract the
// future submission form orchestrates.
#[tokio::test]
async fn upload_and_status_set_are_two_calls_with_their_own_entries() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref sheet" })).await;

    // Call 1 — the upload.
    let part = reqwest::multipart::Part::bytes(b"PNG-BYTES".to_vec())
        .file_name("final.png")
        .mime_str("image/png")
        .expect("valid mime");
    let form = reqwest::multipart::Form::new().part("file", part);
    let res = client
        .post(format!("{base}/commissions/{id}/files"))
        .multipart(form)
        .send()
        .await
        .expect("POST file");
    assert_eq!(res.status(), 201);
    let file_id = res.json::<serde_json::Value>().await.expect("file id")["id"].clone();

    // Call 2 — the explicit Status-set (the ZMVP-85 machinery, untouched).
    let res = client
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "status": "waiting_for_approval" }))
        .send()
        .await
        .expect("PUT direction status");
    assert_eq!(res.status(), 204);
    assert_eq!(
        stored(&backend, id).await.direction_status,
        Some(DirectionStatus::WaitingForApproval),
        "the explicit set applied"
    );

    // Each call produced its own entry: creation, then file_added, then
    // status_changed — three distinct records, never a merged one.
    let log = entries(&backend, id).await;
    let kinds: Vec<&str> = log.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["created", "file_added", "status_changed"],
        "two calls, two entries — plus the genesis"
    );

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("provisioned");
    let file_entry = &log[1];
    assert_eq!(file_entry.actor_id, Some(me.id), "the uploader authored it");
    assert_eq!(
        file_entry.payload["file_id"], file_id,
        "the file entry names its file"
    );
    let status_entry = &log[2];
    assert_eq!(status_entry.actor_id, Some(me.id), "the setter authored it");
    assert!(
        status_entry.payload["from"].is_null(),
        "from: null — the fresh commission held no direction status"
    );
    assert_eq!(status_entry.payload["to"], "waiting_for_approval");
}
