//! ZMVP-87 — the changelog records the commission's events, notes, and linked
//! channel, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the Changelog DD `30408741`):
//!
//! - **AC1 (in-stack slice)** — creation itself appends a `created` entry; the
//!   other emitters (lifecycle, status, seats, transfers, placements, view
//!   grants) land with their own tickets and emit through the same API.
//! - **AC2** — a Participant writes a free-text note into the same stream; a
//!   blank note is rejected.
//! - **AC3** — the owner links/clears the external channel pointer; each act
//!   appends an entry; the pointer is raw text (no scheme allowlist), but
//!   length-capped and control-character-free.
//! - **AC4** — append-only: no route edits or removes an entry.
//! - **AC5** — Participants read the stream in order (seq ascending); a
//!   non-participant gets the **identical** 404 a missing commission gets
//!   (existence-hiding, the closed-door policy — never a 403 oracle).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect what was persisted. `did` is the
/// identity `sign_in` will authenticate as.
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

/// Drives the two-step sign-in so the client's cookie jar carries a live session
/// for the app's configured DID.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=artist.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin should redirect to the PDS");
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback should redirect on success");
}

/// Creates a commission over HTTP as the signed-in caller and returns its id
/// (introspected off the backend — the route returns a bare `201`).
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");
    let all = backend.all_commissions().await.expect("list commissions");
    *all.last().expect("a commission was persisted").id
}

/// Reads the changelog as a JSON array (asserting the `200`).
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
    assert_eq!(res.status(), 200, "a participant reads the changelog");
    res.json().await.expect("changelog body is a JSON array")
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = CommissionTitle::try_new("Not yours").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1 (creation itself) — POST /commissions appends the `created` entry: the
// stream's first entry names the creating User as actor and carries a payload
// that renders without joins.
#[tokio::test]
async fn creating_a_commission_appends_the_creation_entry() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 1, "creation appends exactly one entry");
    let entry = &entries[0];
    assert_eq!(entry["kind"], "created", "the entry is the creation event");
    assert_eq!(
        entry["actor_id"],
        json!(*me.id),
        "the creating User is the actor",
    );
    assert!(entry["seq"].is_i64(), "the ordering key is carried");
    assert!(
        entry["created_at"].is_string(),
        "the display timestamp is carried",
    );
    assert_eq!(
        entry["payload"]["title"], "A ref sheet",
        "the payload renders a sentence without joins",
    );
}

// AC2 — a Participant (the owner) writes a free-text note; it lands in the SAME
// stream, after the creation entry, carrying the note text and the actor.
#[tokio::test]
async fn a_participant_writes_a_note_into_the_stream() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/notes"))
        .json(&json!({ "note": "traveling next week" }))
        .send()
        .await
        .expect("POST note");
    assert_eq!(res.status(), 201, "a note is appended");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 2, "the note joins the same stream");
    assert_eq!(entries[1]["kind"], "note");
    assert_eq!(entries[1]["note"], "traveling next week");
    assert!(
        entries[1]["actor_id"].is_string(),
        "a note is never a system entry",
    );
}

// AC2 floor — a blank (whitespace-only) note is rejected `422` and appends nothing.
#[tokio::test]
async fn a_blank_note_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/notes"))
        .json(&json!({ "note": "   " }))
        .send()
        .await
        .expect("POST blank note");
    common::assert_problem(res, 422, "invalid_request").await;

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 1, "only the creation entry remains");
}

// AC5 — entries read back in append order: seq strictly increases and the
// note texts come back in the order they were written.
#[tokio::test]
async fn entries_read_back_in_append_order() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    for text in ["first", "second", "third"] {
        let res = client
            .post(format!("{base}/commissions/{id}/notes"))
            .json(&json!({ "note": text }))
            .send()
            .await
            .expect("POST note");
        assert_eq!(res.status(), 201);
    }

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 4, "creation + three notes");
    let seqs: Vec<i64> = entries.iter().map(|e| e["seq"].as_i64().unwrap()).collect();
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "seq strictly increases down the stream: {seqs:?}",
    );
    let notes: Vec<&str> = entries[1..]
        .iter()
        .map(|e| e["note"].as_str().unwrap())
        .collect();
    assert_eq!(
        notes,
        ["first", "second", "third"],
        "append order is read order"
    );
}

// AC5 (closed door) — a signed-in NON-participant reading the changelog gets the
// **byte-identical** problem+json a nonexistent commission gets: a 404, never a
// 403, so the response is no existence oracle (ZMVP-75's closed-door AC).
#[tokio::test]
async fn a_non_participant_gets_the_same_404_as_a_missing_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;

    let res = client
        .get(format!("{base}/commissions/{foreign}/changelog"))
        .send()
        .await
        .expect("GET foreign changelog");
    assert_eq!(
        res.status(),
        404,
        "a hidden commission answers 404, never 403"
    );
    let hidden_body: serde_json::Value = res.json().await.expect("problem body");

    let missing = uuid::Uuid::now_v7();
    let res = client
        .get(format!("{base}/commissions/{missing}/changelog"))
        .send()
        .await
        .expect("GET missing changelog");
    assert_eq!(res.status(), 404);
    let missing_body: serde_json::Value = res.json().await.expect("problem body");

    assert_eq!(
        hidden_body, missing_body,
        "hidden and absent commissions are indistinguishable by construction",
    );
    assert_eq!(hidden_body["code"], "commission_not_found");
}

// The write half of the closed door — a non-participant's note and channel
// writes get the same uniform 404, and nothing is appended.
#[tokio::test]
async fn a_non_participant_cannot_write_into_a_hidden_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;

    let res = client
        .post(format!("{base}/commissions/{foreign}/notes"))
        .json(&json!({ "note": "sneaky" }))
        .send()
        .await
        .expect("POST foreign note");
    common::assert_problem(res, 404, "commission_not_found").await;

    let res = client
        .put(format!("{base}/commissions/{foreign}/channel"))
        .json(&json!({ "channel": "https://t.me/mychat" }))
        .send()
        .await
        .expect("PUT foreign channel");
    common::assert_problem(res, 404, "commission_not_found").await;

    assert!(
        backend
            .changelog_entries(domain::elements::commission::CommissionId::new(foreign))
            .await
            .expect("inspect entries")
            .is_empty(),
        "nothing was appended to the hidden commission's stream",
    );
}

// The floor — anonymous callers are turned away with `401` on every surface.
#[tokio::test]
async fn anonymous_callers_are_turned_away() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;

    let anon = client();
    let res = anon
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("anon GET changelog");
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = anon
        .post(format!("{base}/commissions/{id}/notes"))
        .json(&json!({ "note": "hi" }))
        .send()
        .await
        .expect("anon POST note");
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = anon
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "x" }))
        .send()
        .await
        .expect("anon PUT channel");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// AC3 — the owner links the external channel (any raw pointer text), which
// appends a `channel_linked` entry carrying the pointer; clearing appends
// `channel_unlinked`; clearing again is an idempotent no-op (no noise entry).
#[tokio::test]
async fn the_owner_links_and_clears_the_channel() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "https://t.me/refsheet-chat" }))
        .send()
        .await
        .expect("PUT channel");
    assert_eq!(res.status(), 204, "linking the channel succeeds");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1]["kind"], "channel_linked");
    assert_eq!(
        entries[1]["payload"]["channel"], "https://t.me/refsheet-chat",
        "the entry renders the pointer without joins",
    );

    // Re-declaring the identical pointer is a no-op: 204, but no noise entry
    // (the append is keyed on the port's changed answer, inside the unit).
    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "https://t.me/refsheet-chat" }))
        .send()
        .await
        .expect("PUT identical channel again");
    assert_eq!(
        res.status(),
        204,
        "re-linking the same pointer is idempotent"
    );
    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 2, "no entry is appended for a no-op re-link");

    let res = client
        .delete(format!("{base}/commissions/{id}/channel"))
        .send()
        .await
        .expect("DELETE channel");
    assert_eq!(res.status(), 204, "clearing the channel succeeds");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2]["kind"], "channel_unlinked");

    // Clearing an already-clear channel is a no-op: 204, but no noise entry.
    let res = client
        .delete(format!("{base}/commissions/{id}/channel"))
        .send()
        .await
        .expect("DELETE channel again");
    assert_eq!(res.status(), 204, "clearing twice is idempotent");
    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 3, "no entry is appended for a no-op clear");
}

// AC3 validation — the pointer is raw text with NO scheme allowlist (a bare
// handle passes), but control characters and over-cap lengths are rejected.
#[tokio::test]
async fn channel_pointer_is_raw_text_but_capped_and_control_free() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    // No scheme allowlist: a non-URL handle is a fine pointer.
    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "@artist on Telegram" }))
        .send()
        .await
        .expect("PUT handle-shaped channel");
    assert_eq!(
        res.status(),
        204,
        "a non-URL pointer is accepted (renders as text)"
    );

    // Control characters are rejected.
    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "https://t.me/x\nSet-Cookie: no" }))
        .send()
        .await
        .expect("PUT control-char channel");
    common::assert_problem(res, 422, "invalid_request").await;

    // Over the length cap is rejected.
    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "x".repeat(513) }))
        .send()
        .await
        .expect("PUT oversized channel");
    common::assert_problem(res, 422, "invalid_request").await;

    // A blank pointer is rejected (clear via DELETE, not an empty PUT).
    let res = client
        .put(format!("{base}/commissions/{id}/channel"))
        .json(&json!({ "channel": "   " }))
        .send()
        .await
        .expect("PUT blank channel");
    common::assert_problem(res, 422, "invalid_request").await;
}

// AC4 — append-only at the HTTP surface: there is no route that edits or
// removes a changelog entry (or the stream). Method probes on the stream and an
// entry path never find a mutation handler.
#[tokio::test]
async fn no_route_edits_or_removes_changelog_entries() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    for (method, url) in [
        (
            reqwest::Method::PUT,
            format!("{base}/commissions/{id}/changelog"),
        ),
        (
            reqwest::Method::PATCH,
            format!("{base}/commissions/{id}/changelog"),
        ),
        (
            reqwest::Method::DELETE,
            format!("{base}/commissions/{id}/changelog"),
        ),
        (
            reqwest::Method::PUT,
            format!("{base}/commissions/{id}/changelog/1"),
        ),
        (
            reqwest::Method::PATCH,
            format!("{base}/commissions/{id}/changelog/1"),
        ),
        (
            reqwest::Method::DELETE,
            format!("{base}/commissions/{id}/changelog/1"),
        ),
    ] {
        let res = client
            .request(method.clone(), &url)
            .send()
            .await
            .expect("mutation probe");
        assert!(
            matches!(res.status().as_u16(), 404 | 405),
            "{method} {url} must not exist, got {}",
            res.status(),
        );
    }

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 1, "the stream is untouched by the probes");
}
