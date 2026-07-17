//! ZMVP-88 — a Participant uploads a file entry to the changelog, and retrieves
//! one, end to end over HTTP. The stack's security anchor: Total-tier file content
//! served by the API, participant-only, with the download hardened against stored
//! XSS.
//!
//! Pins the acceptance criteria at the API surface (DESIGN/Commission — "File
//! entries and Markup"):
//!
//! - **AC1** — any Participant uploads a file as a `file_added` changelog event
//!   (the payload renders without joins: filename/mime/size).
//! - **AC2** — a file entry does NOT trigger fact-lock (`commission_has_facts`
//!   stays false with only file entries; proven in `adapter-pg/tests` — see the
//!   fact-predicate + tripwire there).
//! - **AC3** — a Participant retrieves a file entry; a non-participant never can
//!   (the uniform closed-door 404), and the download is served `attachment` +
//!   `nosniff` so a stored SVG/HTML never executes in the app origin.
//! - **AC4** — the blob store is a port; v1 ships a mock/local implementation (the
//!   in-memory fake here, a pg `bytea` table in `main`).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// The default upload cap for the suite (25 MiB, matching production). The size-cap
/// test overrides it with a tiny value.
const DEFAULT_MAX_UPLOAD: u64 = 25 * 1024 * 1024;

/// Boots the app with everything faked in-process, at the given upload cap; returns
/// the base URL and the [`MemBackend`] for introspection. `did` is the identity
/// `sign_in` authenticates as.
async fn spawn_app(did: &str, max_upload_bytes: u64) -> (String, MemBackend) {
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
            max_upload_bytes,
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

/// Creates a commission over HTTP as the signed-in caller and returns its id,
/// resolved by its (unique) title — the route returns a bare `201`, and
/// `all_commissions` is unordered, so a title lookup is the reliable way to pick a
/// specific one out when a test creates several.
async fn create_commission_titled(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
    title: &str,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": title }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201);
    let all = backend.all_commissions().await.expect("list commissions");
    *all.iter()
        .find(|c| c.title.as_str() == title)
        .expect("the just-created commission is persisted")
        .id
}

/// The single-commission convenience for tests that only make one.
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    create_commission_titled(client, base, backend, "A ref sheet").await
}

/// Uploads `bytes` as the `file` part with the given filename/mime, returning the
/// raw response for the caller to assert on.
async fn upload(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str(mime)
        .expect("valid mime");
    let form = reqwest::multipart::Form::new().part("file", part);
    client
        .post(format!("{base}/commissions/{commission}/files"))
        .multipart(form)
        .send()
        .await
        .expect("POST file")
}

/// Reads the changelog as a JSON array.
async fn read_changelog(
    client: &reqwest::Client,
    base: &str,
    id: uuid::Uuid,
) -> Vec<serde_json::Value> {
    let res = client
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("GET changelog");
    assert_eq!(res.status(), 200);
    res.json().await.expect("changelog is a JSON array")
}

/// Seeds a committed commission owned by someone other than the signed-in caller.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let commission = Commission::create(
        "Not yours".parse::<CommissionTitle>().expect("title"),
        owner.id,
        Utc::now(),
        None,
    );
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1 — a Participant uploads a file; it lands as a `file_added` changelog event
// whose payload renders a sentence without joins (filename/mime/size), authored by
// the uploader, and the upload returns the new file id.
#[tokio::test]
async fn a_participant_uploads_a_file_as_a_changelog_event() {
    let (base, backend) = spawn_app("did:plc:artist", DEFAULT_MAX_UPLOAD).await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = upload(
        &client,
        &base,
        id,
        "sketch.png",
        "image/png",
        b"PNG-BYTES".to_vec(),
    )
    .await;
    assert_eq!(res.status(), 201, "upload returns 201");
    let body: serde_json::Value = res.json().await.expect("upload returns the file id");
    assert!(
        body["id"].as_str().is_some(),
        "the new file id is returned: {body:?}"
    );

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("provisioned");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 2, "creation + file_added");
    let entry = &entries[1];
    assert_eq!(entry["kind"], "file_added");
    assert_eq!(
        entry["actor_id"],
        json!(*me.id),
        "the uploader is the actor"
    );
    assert_eq!(entry["payload"]["filename"], "sketch.png");
    assert_eq!(entry["payload"]["content_type"], "image/png");
    assert_eq!(entry["payload"]["byte_size"], json!(9));
    assert_eq!(
        entry["payload"]["file_id"], body["id"],
        "the entry names the file it added",
    );
}

// AC3 — a Participant retrieves the file: the exact bytes come back, with
// Content-Type from the upload AND the always-on download hardening
// (Content-Disposition: attachment + X-Content-Type-Options: nosniff).
#[tokio::test]
async fn a_participant_retrieves_the_file_with_hardened_headers() {
    let (base, backend) = spawn_app("did:plc:artist", DEFAULT_MAX_UPLOAD).await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let content = b"the file contents".to_vec();
    let res = upload(
        &client,
        &base,
        id,
        "ref.bin",
        "application/octet-stream",
        content.clone(),
    )
    .await;
    let file_id = res.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = client
        .get(format!("{base}/commissions/{id}/files/{file_id}"))
        .send()
        .await
        .expect("GET file");
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "application/octet-stream",
    );
    assert_eq!(
        res.headers().get("x-content-type-options").unwrap(),
        "nosniff",
        "downloads must forbid MIME sniffing",
    );
    let disposition = res
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.starts_with("attachment"),
        "downloads are always attachments, never inline: {disposition}",
    );
    assert!(
        disposition.contains("ref.bin"),
        "the filename hint is carried: {disposition}"
    );
    let got = res.bytes().await.unwrap();
    assert_eq!(
        got.as_ref(),
        content.as_slice(),
        "the exact bytes round-trip"
    );
}

// AC3 (the security anchor) — a stored SVG carrying a script is served as an inert
// attachment with nosniff, byte-for-byte: it can never execute in the app origin.
#[tokio::test]
async fn a_stored_svg_is_served_inert_not_executed() {
    let (base, backend) = spawn_app("did:plc:artist", DEFAULT_MAX_UPLOAD).await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let svg =
        br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#.to_vec();
    let res = upload(&client, &base, id, "evil.svg", "image/svg+xml", svg.clone()).await;
    let file_id = res.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = client
        .get(format!("{base}/commissions/{id}/files/{file_id}"))
        .send()
        .await
        .expect("GET svg");
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(
        res.headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("attachment"),
        "an SVG is downloaded, never rendered inline",
    );
    assert_eq!(
        res.bytes().await.unwrap().as_ref(),
        svg.as_slice(),
        "the bytes are returned verbatim, not interpreted",
    );
}

// AC3 (closed door) — a non-participant can neither upload to nor retrieve from a
// hidden commission: both get the uniform commission-not-found 404 (never a 403
// oracle), and nothing is appended.
#[tokio::test]
async fn a_non_participant_cannot_upload_or_retrieve() {
    let (base, backend) = spawn_app("did:plc:outsider", DEFAULT_MAX_UPLOAD).await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;

    let res = upload(
        &client,
        &base,
        foreign,
        "sneaky.png",
        "image/png",
        b"x".to_vec(),
    )
    .await;
    common::assert_problem(res, 404, "commission_not_found").await;

    // A guessed file id under a hidden commission is the same 404.
    let guessed = uuid::Uuid::now_v7();
    let res = client
        .get(format!("{base}/commissions/{foreign}/files/{guessed}"))
        .send()
        .await
        .expect("GET foreign file");
    common::assert_problem(res, 404, "commission_not_found").await;

    assert!(
        backend
            .changelog_entries(CommissionId::new(foreign))
            .await
            .expect("entries")
            .is_empty(),
        "nothing was appended to the hidden commission",
    );
}

// AC3 (no cross-commission oracle) — a participant of commission A cannot fetch a
// file that belongs to commission B by pathing it through A: it is file_not_found,
// indistinguishable from a truly absent id, so retrieval is no cross-commission
// existence oracle.
#[tokio::test]
async fn a_file_is_invisible_across_commissions() {
    let (base, backend) = spawn_app("did:plc:artist", DEFAULT_MAX_UPLOAD).await;
    let client = client();
    sign_in(&client, &base).await;
    let mine = create_commission_titled(&client, &base, &backend, "Mine").await;
    let other = create_commission_titled(&client, &base, &backend, "Other").await;

    // Upload into `other`.
    let res = upload(&client, &base, other, "b.png", "image/png", b"B".to_vec()).await;
    let file_in_other = res.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Ask for it via `mine` — I own both, so the gate passes; the file simply isn't
    // in `mine`, so it is file_not_found, exactly like an absent id.
    let cross = client
        .get(format!("{base}/commissions/{mine}/files/{file_in_other}"))
        .send()
        .await
        .expect("cross GET");
    common::assert_problem(cross, 404, "file_not_found").await;

    let absent = client
        .get(format!(
            "{base}/commissions/{mine}/files/{}",
            uuid::Uuid::now_v7()
        ))
        .send()
        .await
        .expect("absent GET");
    common::assert_problem(absent, 404, "file_not_found").await;

    // And it IS retrievable via its own commission (sanity: the gate isn't over-tight).
    let ok = client
        .get(format!("{base}/commissions/{other}/files/{file_in_other}"))
        .send()
        .await
        .expect("own GET");
    assert_eq!(ok.status(), 200);
}

// AC3 floor — anonymous callers are turned away 401 on both surfaces.
#[tokio::test]
async fn anonymous_callers_are_turned_away() {
    let (base, backend) = spawn_app("did:plc:artist", DEFAULT_MAX_UPLOAD).await;
    let signed = client();
    sign_in(&signed, &base).await;
    let id = create_commission(&signed, &base, &backend).await;

    let anon = client();
    let res = upload(&anon, &base, id, "a.png", "image/png", b"x".to_vec()).await;
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = anon
        .get(format!(
            "{base}/commissions/{id}/files/{}",
            uuid::Uuid::now_v7()
        ))
        .send()
        .await
        .expect("anon GET");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Upload validation — an oversize file is 413, an empty file is 422, and a
// path-separator filename is 422 (a header/path-injection vector), appending nothing.
#[tokio::test]
async fn upload_is_capped_and_validated() {
    let (base, backend) = spawn_app("did:plc:artist", 1024).await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    // Over the 1024-byte cap (but under the framework body limit) → 413.
    let big = vec![b'x'; 4096];
    let res = upload(
        &client,
        &base,
        id,
        "big.bin",
        "application/octet-stream",
        big,
    )
    .await;
    common::assert_problem(res, 413, "payload_too_large").await;

    // Empty file → 422.
    let res = upload(
        &client,
        &base,
        id,
        "empty.bin",
        "application/octet-stream",
        vec![],
    )
    .await;
    common::assert_problem(res, 422, "invalid_request").await;

    // Path-separator filename → 422.
    let res = upload(
        &client,
        &base,
        id,
        "../../etc/passwd",
        "text/plain",
        b"x".to_vec(),
    )
    .await;
    common::assert_problem(res, 422, "invalid_request").await;

    assert_eq!(
        read_changelog(&client, &base, id).await.len(),
        1,
        "only the creation entry remains — no rejected upload was recorded",
    );
}
