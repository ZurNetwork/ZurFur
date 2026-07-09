//! ZMVP-85 — a Participant sets the direction-axis Status explicitly, end to
//! end over HTTP.
//!
//! Pins the acceptance criteria at the API surface (DESIGN/Commission, Status;
//! Engineer ruling E29 — one nullable column, so axis exclusivity falls out of
//! the shape):
//!
//! - **AC1** — a Participant sets a direction status (Waiting for Input /
//!   Waiting for Approval / Changes Requested) or clears it; each change
//!   appends a `status_changed` entry.
//! - **AC2** — direction-axis values are mutually exclusive: setting one
//!   REPLACES the current one (never accumulates).
//! - **AC3** — direction and deadline axes compose freely: the direction
//!   status never touches the deadline envelope field (the deadline-axis
//!   machinery itself is ZMVP-86).
//! - **AC4** — no content event changes a direction status by itself: a note
//!   (the content event that exists today) leaves it untouched; the only
//!   writer of the column is this explicit endpoint, so the future markup/file
//!   emitters (ZMVP-88/90) inherit the rule by construction.
//! - **Closed door** — a non-participant (who may not learn the commission
//!   exists) gets the byte-identical 404 a missing commission gets, never a
//!   403; anonymous callers get a 401.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, DirectionStatus},
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

/// Creates a commission over HTTP as the signed-in caller (optionally with a
/// deadline) and returns its id (introspected off the backend — the route
/// returns a bare `201`).
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

/// The persisted direction status of `id`, as its stable wire token.
async fn stored_status(backend: &MemBackend, id: uuid::Uuid) -> Option<&'static str> {
    backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists")
        .direction_status
        .map(|s| s.as_str())
}

/// PUT the direction status and return the response.
async fn put_status(
    client: &reqwest::Client,
    base: &str,
    id: uuid::Uuid,
    status: &str,
) -> reqwest::Response {
    client
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "status": status }))
        .send()
        .await
        .expect("PUT direction status")
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

// AC1 — a Participant (the owner) sets each of the three direction statuses;
// the column holds the value and each change appends a `status_changed` entry
// naming the actor and the from/to values (a sentence without joins).
#[tokio::test]
async fn a_participant_sets_each_direction_status() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    assert_eq!(
        stored_status(&backend, id).await,
        None,
        "a fresh commission carries no direction status"
    );

    for token in [
        "waiting_for_input",
        "waiting_for_approval",
        "changes_requested",
    ] {
        let res = put_status(&client, &base, id, token).await;
        assert_eq!(res.status(), 204, "setting {token} succeeds");
        assert_eq!(stored_status(&backend, id).await, Some(token));
    }

    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 4, "creation + three status changes");
    let last = &log[3];
    assert_eq!(last.kind.as_str(), "status_changed");
    assert!(
        last.actor_id.is_some(),
        "an explicit set is never a system entry"
    );
    assert_eq!(
        last.payload["from"], "waiting_for_approval",
        "the entry names the replaced value"
    );
    assert_eq!(last.payload["to"], "changes_requested");
}

// AC2 — mutual exclusivity by shape: setting a second value REPLACES the first;
// the commission never holds two direction values.
#[tokio::test]
async fn setting_a_direction_status_replaces_the_current_one() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    assert_eq!(
        put_status(&client, &base, id, "waiting_for_input")
            .await
            .status(),
        204
    );
    assert_eq!(
        put_status(&client, &base, id, "waiting_for_approval")
            .await
            .status(),
        204
    );

    // One nullable column: the old value is gone, the new one is THE value.
    assert_eq!(
        stored_status(&backend, id).await,
        Some("waiting_for_approval"),
        "the set replaced, not accumulated"
    );
}

// AC1 (clear) — a Participant clears the direction status: the column goes
// NULL and the clear itself is changelog-recorded (`to: null`); clearing an
// already-clear status is an idempotent no-op that appends no noise entry.
#[tokio::test]
async fn a_participant_clears_the_direction_status() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    assert_eq!(
        put_status(&client, &base, id, "changes_requested")
            .await
            .status(),
        204
    );

    let res = client
        .delete(format!("{base}/commissions/{id}/status/direction"))
        .send()
        .await
        .expect("DELETE direction status");
    assert_eq!(res.status(), 204, "clearing succeeds");
    assert_eq!(stored_status(&backend, id).await, None, "cleared to NULL");

    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 3, "creation + set + clear");
    let clear = &log[2];
    assert_eq!(clear.kind.as_str(), "status_changed");
    assert_eq!(clear.payload["from"], "changes_requested");
    assert!(clear.payload["to"].is_null(), "a clear records to: null");

    // Clearing again changes nothing: 204, but no noise entry.
    let res = client
        .delete(format!("{base}/commissions/{id}/status/direction"))
        .send()
        .await
        .expect("DELETE direction status again");
    assert_eq!(res.status(), 204, "clearing twice is idempotent");
    assert_eq!(
        entries(&backend, id).await.len(),
        3,
        "no entry is appended for a no-op clear"
    );
}

// Re-setting the value already held is the set-side no-op: 204, no noise entry.
#[tokio::test]
async fn re_setting_the_same_status_appends_no_entry() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    assert_eq!(
        put_status(&client, &base, id, "waiting_for_input")
            .await
            .status(),
        204
    );
    assert_eq!(
        put_status(&client, &base, id, "waiting_for_input")
            .await
            .status(),
        204,
        "a same-value set is idempotent"
    );

    assert_eq!(
        entries(&backend, id).await.len(),
        2,
        "creation + one status change: nothing changed the second time"
    );
    assert_eq!(stored_status(&backend, id).await, Some("waiting_for_input"));
}

// AC3 — the axes compose freely: a commission with a deadline takes a direction
// status without the deadline moving, and clearing the status leaves it alone.
// (The deadline-axis values themselves — Delayed/Late — are ZMVP-86.)
#[tokio::test]
async fn direction_status_composes_with_the_deadline_axis() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-01-01T00:00:00Z" }),
    )
    .await;

    assert_eq!(
        put_status(&client, &base, id, "waiting_for_approval")
            .await
            .status(),
        204
    );

    let commission = backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(
        commission.direction_status.map(|s| s.as_str().to_owned()),
        Some("waiting_for_approval".to_owned())
    );
    assert!(
        commission.deadline.is_some(),
        "the deadline envelope is untouched by a direction set (the axes compose)"
    );

    let res = client
        .delete(format!("{base}/commissions/{id}/status/direction"))
        .send()
        .await
        .expect("DELETE direction status");
    assert_eq!(res.status(), 204);
    let commission = backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find")
        .expect("exists");
    assert!(
        commission.deadline.is_some(),
        "clearing the status never clears the deadline"
    );
}

// AC4 — no content event changes a direction status by itself: a note (the
// content event that exists today) is written and the status stays exactly
// where the participant put it. The column's only writer is the explicit
// endpoint, so future content emitters (markup ZMVP-90, file entries ZMVP-88)
// inherit this by construction.
#[tokio::test]
async fn a_content_event_never_moves_the_direction_status() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    assert_eq!(
        put_status(&client, &base, id, "waiting_for_input")
            .await
            .status(),
        204
    );

    let res = client
        .post(format!("{base}/commissions/{id}/notes"))
        .json(&json!({ "note": "here is the sketch, thoughts?" }))
        .send()
        .await
        .expect("POST note");
    assert_eq!(res.status(), 201, "the note lands");

    assert_eq!(
        stored_status(&backend, id).await,
        Some("waiting_for_input"),
        "a content event never mutates the direction status"
    );
}

// Validation — a token outside the three-value vocabulary, and a malformed
// body, are each a 422 with nothing stored or appended.
#[tokio::test]
async fn an_unknown_status_token_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    let res = put_status(&client, &base, id, "on_fire").await;
    common::assert_problem(res, 422, "invalid_request").await;

    let res = client
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "wrong": "shape" }))
        .send()
        .await
        .expect("PUT malformed body");
    common::assert_problem(res, 422, "invalid_request").await;

    assert_eq!(stored_status(&backend, id).await, None, "nothing stored");
    assert_eq!(
        entries(&backend, id).await.len(),
        1,
        "only the creation entry remains"
    );
}

// Closed door — a signed-in NON-participant setting or clearing the status gets
// the **byte-identical** problem+json a nonexistent commission gets: a 404,
// never a 403 (no existence oracle), and nothing changes.
#[tokio::test]
async fn a_non_participant_gets_the_same_404_as_a_missing_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;

    let res = put_status(&client, &base, foreign, "waiting_for_input").await;
    assert_eq!(
        res.status(),
        404,
        "a hidden commission answers 404, never 403"
    );
    let hidden_body: serde_json::Value = res.json().await.expect("problem body");

    let missing = uuid::Uuid::now_v7();
    let res = put_status(&client, &base, missing, "waiting_for_input").await;
    assert_eq!(res.status(), 404);
    let missing_body: serde_json::Value = res.json().await.expect("problem body");

    assert_eq!(
        hidden_body, missing_body,
        "hidden and absent commissions are indistinguishable by construction",
    );
    assert_eq!(hidden_body["code"], "commission_not_found");

    let res = client
        .delete(format!("{base}/commissions/{foreign}/status/direction"))
        .send()
        .await
        .expect("DELETE foreign status");
    common::assert_problem(res, 404, "commission_not_found").await;

    assert_eq!(
        stored_status(&backend, foreign).await,
        None,
        "the hidden commission's status never moved"
    );
    assert!(
        entries(&backend, foreign).await.is_empty(),
        "nothing was appended to the hidden commission's stream"
    );
}

// The floor — anonymous callers are turned away with `401` on both methods.
#[tokio::test]
async fn anonymous_callers_are_turned_away() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend, json!({ "title": "Ref" })).await;

    let anon = client();
    let res = anon
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "status": "waiting_for_input" }))
        .send()
        .await
        .expect("anon PUT status");
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = anon
        .delete(format!("{base}/commissions/{id}/status/direction"))
        .send()
        .await
        .expect("anon DELETE status");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// The three wire tokens and only they are accepted end to end — pinning the
// vocabulary at the boundary (the enum owns it; DirectionStatus::ALL is the
// closed set).
#[tokio::test]
async fn the_vocabulary_is_exactly_the_three_direction_values() {
    assert_eq!(
        DirectionStatus::ALL
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        vec![
            "waiting_for_input",
            "waiting_for_approval",
            "changes_requested"
        ],
    );
}
