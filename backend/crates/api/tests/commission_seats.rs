//! ZMVP-76 — the owner declares a Seat on the commission, over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams —
//! the persisted participant row, the node+satellite atomicity, the parent
//! gate — are covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner declares a Seat with a typed kind via
//!   `POST /commissions/{id}/seats`; a commission holds several Seats, kinds
//!   repeating freely (an open vocabulary — ruling E21: not the Role enum).
//! - **AC2** — a vacant Seat carries requirements: a free-text prompt and/or
//!   an external link (the v1 vocabulary; both optional, no form builder).
//! - **AC3** — a Seat holds at most one occupant: every declared seat is born
//!   vacant (occupancy is a single slot; filling it is ZMVP-79).
//! - **AC4** — a vacant Seat under Description-visible surfaces appears in the
//!   non-participant projection: **deferred** — the projection is ZMVP-75,
//!   which is not in this stack's lineage yet; the `#[ignore]`d test at the
//!   bottom documents the criterion for the post-rebase arm.
//! - The declaration is changelog-recorded (`seat_declared`, ZMVP-87's frozen
//!   taxonomy) atomically with the seat.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a
//!   403; a fabricated/foreign parent is a `node_not_found` 404; a component
//!   parent is a `409`; a malformed body or invalid kind/prompt/link is `422`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{ChangelogEntryKind, Commission, CommissionId, CommissionTitle},
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

/// Drives the two-step sign-in so the client's cookie jar carries a live
/// session for the app's configured DID.
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

/// The commission's root node id, introspected off the backend.
async fn root_of(backend: &MemBackend, commission: uuid::Uuid) -> uuid::Uuid {
    *backend
        .commission_store()
        .load_tree(CommissionId::new(commission))
        .await
        .expect("load tree")
        .expect("every commission has a tree")
        .root
        .id
}

/// POSTs a seat declaration and returns the created seat's node id from the
/// `201` body.
async fn declare_seat(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    body: &serde_json::Value,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions/{commission}/seats"))
        .json(body)
        .send()
        .await
        .expect("POST seat");
    assert_eq!(res.status(), 201, "declaring a seat returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new seat's node id")
        .parse()
        .expect("the id is a UUID")
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1/AC2/AC3 — the owner declares seats with typed kinds and requirements:
// each lands as a vacant satellite keyed by the returned node id, kinds repeat
// freely across the commission, requirements are optional per seat, and the
// declaration appends the seat_declared changelog entry atomically.
#[tokio::test]
async fn the_owner_declares_seats_with_kinds_repeating_freely() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let first = declare_seat(
        &client,
        &base,
        id,
        &json!({
            "parent": root,
            "kind": "Creator",
            "prompt": "Two refs, please.",
            "link": "https://forms.example/apply",
        }),
    )
    .await;
    // Same kind again — kinds repeat freely (AC1). No requirements — both
    // optional (AC2).
    let second = declare_seat(
        &client,
        &base,
        id,
        &json!({ "parent": root, "kind": "Creator" }),
    )
    .await;

    let seats = backend
        .commission_store()
        .seats(CommissionId::new(id))
        .await
        .expect("seats");
    assert_eq!(seats.len(), 2, "a commission holds several Seats (AC1)");
    let first_seat = seats
        .iter()
        .find(|s| *s.id == first)
        .expect("the 201 id reappears as a seat");
    assert_eq!(first_seat.kind.as_str(), "Creator");
    assert_eq!(
        first_seat.prompt.as_ref().map(|p| p.as_str()),
        Some("Two refs, please."),
        "the prompt rides the vacant seat (AC2)"
    );
    assert_eq!(
        first_seat.link.as_ref().map(|l| l.as_str()),
        Some("https://forms.example/apply"),
        "the link rides the vacant seat (AC2)"
    );
    assert!(first_seat.is_vacant(), "born vacant (AC3)");
    let second_seat = seats.iter().find(|s| *s.id == second).expect("second seat");
    assert_eq!(second_seat.kind.as_str(), "Creator", "kinds repeat (AC1)");
    assert!(second_seat.prompt.is_none() && second_seat.link.is_none());
    assert!(second_seat.is_vacant());

    // The seat's node is in the tree, under the root.
    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(
        tree.root.children.len(),
        2,
        "seats are components in the tree"
    );

    // The declarations are changelog-recorded: creation + two seat_declared.
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(entries.len(), 3);
    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");
    for entry in &entries[1..] {
        assert!(matches!(entry.kind, ChangelogEntryKind::SeatDeclared));
        assert_eq!(entry.actor_id, Some(me.id), "the owner is the actor");
        assert_eq!(
            entry.payload["kind"], "Creator",
            "the payload renders a sentence without joins"
        );
    }
}

// Floor — anonymous callers can't declare seats: 401, and nothing lands.
#[tokio::test]
async fn an_anonymous_caller_cannot_declare_a_seat() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": root, "kind": "Creator" }))
        .send()
        .await
        .expect("anonymous POST");
    common::assert_problem(res, 401, "not_authenticated").await;
    assert!(
        backend
            .commission_store()
            .seats(CommissionId::new(id))
            .await
            .expect("seats")
            .is_empty()
    );
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;

    let hidden = client
        .post(format!("{base}/commissions/{foreign}/seats"))
        .json(&json!({ "parent": foreign_root, "kind": "Creator" }))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/seats"))
        .json(&json!({ "parent": foreign_root, "kind": "Creator" }))
        .send()
        .await
        .expect("probe absent");
    let absent_status = absent.status().as_u16();
    let absent_body: serde_json::Value = absent.json().await.expect("problem body");

    assert_eq!(hidden_status, 404, "hidden = not found, never 403");
    assert_eq!(hidden_body["code"], "commission_not_found");
    assert_eq!(
        (hidden_status, &hidden_body),
        (absent_status, &absent_body),
        "hidden and absent are indistinguishable (no existence oracle)"
    );
    assert!(
        backend
            .commission_store()
            .seats(CommissionId::new(foreign))
            .await
            .expect("seats")
            .is_empty(),
        "the probe declared nothing"
    );
}

// Floor — the owner naming a parent node that doesn't exist in this commission
// (fabricated, or belonging to another tree) gets node_not_found; a component
// parent gets the honest 409 (seats live under surfaces).
#[tokio::test]
async fn parent_gates_hold_for_seats() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    // Fabricated parent id.
    let res = client
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": uuid::Uuid::now_v7(), "kind": "Creator" }))
        .send()
        .await
        .expect("POST fabricated parent");
    common::assert_problem(res, 404, "node_not_found").await;

    // A real node — in someone else's tree.
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": foreign_root, "kind": "Creator" }))
        .send()
        .await
        .expect("POST foreign parent");
    common::assert_problem(res, 404, "node_not_found").await;

    // A component parent: seats are components — leaves live under surfaces.
    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": root }))
        .send()
        .await
        .expect("POST component");
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.expect("201 body");
    let component = body["id"].as_str().expect("component id");
    let res = client
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": component, "kind": "Creator" }))
        .send()
        .await
        .expect("POST seat under component");
    common::assert_problem(res, 409, "parent_not_a_surface").await;
}

// Floor — a malformed body (no kind / no parent) and an invalid kind, prompt,
// or link are each a 422, and nothing lands.
#[tokio::test]
async fn malformed_and_invalid_bodies_are_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    for body in [
        json!({ "parent": root }),                                    // no kind
        json!({ "kind": "Creator" }),                                 // no parent
        json!({ "parent": root, "kind": "   " }),                     // blank kind
        json!({ "parent": root, "kind": "a\nb" }),                    // control char in kind
        json!({ "parent": root, "kind": "x".repeat(65) }),            // kind too long
        json!({ "parent": root, "kind": "Creator", "prompt": " " }),  // blank prompt
        json!({ "parent": root, "kind": "Creator", "link": "a\nb" }), // control char in link
    ] {
        let res = client
            .post(format!("{base}/commissions/{id}/seats"))
            .json(&body)
            .send()
            .await
            .expect("POST invalid seat");
        common::assert_problem(res, 422, "invalid_request").await;
    }

    assert!(
        backend
            .commission_store()
            .seats(CommissionId::new(id))
            .await
            .expect("seats")
            .is_empty(),
        "no refused declaration landed"
    );
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(entries.len(), 1, "no refused declaration was recorded");
}

// AC4 (DEFERRED — ZMVP-75 is not in this stack's lineage yet): a vacant Seat
// under a Description-visible surface appears in the NON-participant
// projection — the published ask — and a seat under a hidden surface does not.
// The store-side hook this projection consumes (CommissionStore::seats, keyed
// by node id against the projected tree) ships with this ticket; the viewer
// projection endpoint itself lands with ZMVP-75. Re-enable and finish this
// test in the post-rebase arm (it asserts against the projection read).
#[tokio::test]
#[ignore = "ZMVP-76 AC4 is distributed: needs the ZMVP-75 projection (post-rebase arm) — do not count as green"]
async fn a_vacant_seat_under_a_description_surface_is_the_published_ask() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let owner_client = client();
    sign_in(&owner_client, &base).await;
    let id = create_commission(&owner_client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    declare_seat(
        &owner_client,
        &base,
        id,
        &json!({ "parent": root, "kind": "Creator" }),
    )
    .await;

    // POST-REBASE: widen the commission/root to Description visibility
    // (ZMVP-74), then read the non-participant projection (ZMVP-75) as a
    // second, non-participant session and assert the vacant seat (kind +
    // requirements, no occupant) appears — and that a seat under a
    // Total-mode surface does NOT.
    let outsider = client();
    let res = outsider
        .get(format!("{base}/commissions/{id}"))
        .send()
        .await
        .expect("GET projection");
    assert_eq!(
        res.status(),
        200,
        "the ZMVP-75 projection endpoint serves the ask"
    );
    let body: serde_json::Value = res.json().await.expect("projection body");
    assert!(
        body.to_string().contains("Creator"),
        "the vacant seat is the published ask (AC4)"
    );
}
