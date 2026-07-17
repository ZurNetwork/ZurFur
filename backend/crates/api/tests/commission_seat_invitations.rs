//! ZMVP-78 — the owner invites a User to a Seat (issue + revoke), over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams — the
//! partial unique index, the pending→revoked flip — are covered in
//! `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner invites a User to a vacant Seat via
//!   `POST /commissions/{id}/invitations`; a pending offer is recorded (`201`).
//! - **AC2** — a Seat that is already occupied cannot be invited to
//!   (`409 seat_filled`).
//! - **AC3** — re-inviting an already-pending User to the same seat is
//!   idempotent (`200`, the existing offer), never a second row.
//! - **AC4** — a Golem cannot be invited: satisfied by construction (an invitee
//!   is a User by DID; no golem ActorKind exists — DD 34013187), so there is no
//!   representable Golem invitee to reject and no test can reach one.
//! - The owner revokes a pending offer (`200`); revoking nothing pending is an
//!   idempotent `200` no-op.
//! - The floors: anonymous is `401`; a participant who is not the owner is `403`;
//!   an unknown/cross-commission seat is a `404 node_not_found`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, NodeId},
    did::Did,
    profile::Profile,
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

/// Creates a commission over HTTP as the signed-in caller and returns its id.
///
/// `all_commissions()` iterates a `HashMap` (unordered), so `.last()` is not
/// "the newest" once a test holds more than one commission — the new id is found
/// by set-difference against what existed before the call.
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    let before: std::collections::HashSet<uuid::Uuid> = backend
        .all_commissions()
        .await
        .expect("list commissions")
        .iter()
        .map(|c| *c.id)
        .collect();
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");
    backend
        .all_commissions()
        .await
        .expect("list commissions")
        .iter()
        .map(|c| *c.id)
        .find(|id| !before.contains(id))
        .expect("exactly one new commission was persisted")
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

/// Declares a vacant seat over HTTP and returns its node id.
async fn declare_seat(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions/{commission}/seats"))
        .json(&json!({ "parent": parent, "kind": "Creator" }))
        .send()
        .await
        .expect("POST seat");
    assert_eq!(res.status(), 201, "declaring a seat returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body");
    body["id"].as_str().expect("seat id").parse().expect("uuid")
}

// AC1 — the owner invites a User to a vacant seat and a pending offer is recorded.
#[tokio::test]
async fn owner_invites_a_user_and_a_pending_invitation_is_recorded() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&client, &base, id, root).await;

    let res = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("POST invite");
    assert_eq!(res.status(), 201, "issuing a seat invitation returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body");
    assert_eq!(body["seat"], seat.to_string());
    assert_eq!(body["user"], "did:plc:invitee");
    assert_eq!(body["state"], "pending");

    // The offer is queryable through the store.
    let invitee = backend
        .find_by_did(&Did::new("did:plc:invitee".to_string()))
        .await
        .expect("find invitee")
        .expect("the invitee was provisioned");
    let found = backend
        .commission_store()
        .find_pending_seat_invitation(CommissionId::new(id), NodeId::new(seat), invitee.id)
        .await
        .expect("query")
        .expect("a pending offer was recorded");
    assert_eq!(found.state.as_str(), "pending");
    assert_eq!(*found.seat, seat);
}

// AC2 — a Seat that is already occupied cannot be invited to (409 seat_filled).
#[tokio::test]
async fn inviting_to_a_filled_seat_is_a_conflict() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&client, &base, id, root).await;

    // Fill the seat (stand-in for ZMVP-79's accept).
    let occupant = backend
        .provision(&Did::new("did:plc:occupant".to_string()))
        .await
        .expect("provision occupant");
    backend.occupy_seat(NodeId::new(seat), occupant.id);

    let res = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("POST invite to filled seat");
    common::assert_problem(res, 409, "seat_filled").await;
}

// Floor — inviting to a seat that isn't one of this commission's seats
// (fabricated) is a node_not_found 404.
#[tokio::test]
async fn inviting_to_an_unknown_seat_is_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": uuid::Uuid::now_v7(), "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("POST invite fabricated seat");
    common::assert_problem(res, 404, "node_not_found").await;
}

// Floor — a seat that belongs to a DIFFERENT commission is a node_not_found 404
// (the seats read is commission-scoped, so it is no cross-commission oracle).
#[tokio::test]
async fn a_seat_from_another_commission_is_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let target = create_commission(&client, &base, &backend).await;
    // A second commission (mine), with a real seat of its own.
    let other = create_commission(&client, &base, &backend).await;
    let other_root = root_of(&backend, other).await;
    let other_seat = declare_seat(&client, &base, other, other_root).await;

    let res = client
        .post(format!("{base}/commissions/{target}/invitations"))
        .json(&json!({ "seat": other_seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("POST invite cross-commission seat");
    common::assert_problem(res, 404, "node_not_found").await;
}

// AC3 — re-inviting an already-pending User to the same seat is idempotent: the
// existing offer is returned (200), never a second row.
#[tokio::test]
async fn re_inviting_a_pending_user_is_idempotent() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&client, &base, id, root).await;

    let first = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("first invite");
    assert_eq!(first.status(), 201);
    let first_body: serde_json::Value = first.json().await.expect("body");

    let again = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("second invite");
    assert_eq!(
        again.status(),
        200,
        "a re-invite returns the existing offer"
    );
    let again_body: serde_json::Value = again.json().await.expect("body");
    assert_eq!(
        again_body["id"], first_body["id"],
        "the same offer is returned, not a fresh one"
    );
    assert_eq!(again_body["state"], "pending");
}

// The owner revokes a pending offer (200), and it is no longer pending.
#[tokio::test]
async fn owner_revokes_a_pending_invitation() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&client, &base, id, root).await;

    let res = client
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("invite");
    assert_eq!(res.status(), 201);

    let res = client
        .delete(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("revoke");
    assert_eq!(res.status(), 200, "revoking a pending offer returns 200");

    let invitee = backend
        .find_by_did(&Did::new("did:plc:invitee".to_string()))
        .await
        .expect("find")
        .expect("provisioned");
    assert!(
        backend
            .commission_store()
            .find_pending_seat_invitation(CommissionId::new(id), NodeId::new(seat), invitee.id)
            .await
            .expect("query")
            .is_none(),
        "the offer is no longer pending after a revoke"
    );
}

// Revoking with nothing pending (an unknown DID) is an idempotent 200 no-op.
#[tokio::test]
async fn revoking_with_nothing_pending_is_a_no_op() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&client, &base, id, root).await;

    let res = client
        .delete(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:never-invited" }))
        .send()
        .await
        .expect("revoke nothing");
    assert_eq!(
        res.status(),
        200,
        "revoking with nothing pending is a no-op 200"
    );
}

// Floor — a participant who is NOT the owner cannot invite (403). Seeds a
// foreign-owned commission with the signed-in user seated as a (non-owner)
// participant, so require_owner's participant-but-not-owner arm is exercised.
#[tokio::test]
async fn a_participant_who_is_not_owner_cannot_invite() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let artist = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find artist")
        .expect("signed in provisions the artist");

    // A commission owned by someone else, with the artist seated as a participant.
    let foreign_owner = backend
        .provision(&Did::new("did:plc:foreign-owner".to_string()))
        .await
        .expect("provision foreign owner");
    let title = CommissionTitle::try_new("Not yours").expect("valid title");
    let foreign = Commission::create(title, foreign_owner.id, Utc::now(), None);
    let foreign_id = *foreign.id;
    backend
        .create_commission(&foreign)
        .await
        .expect("seed foreign commission");
    backend.seed_participant(foreign.id, artist.id);

    // The seat need not exist — require_owner refuses before the seat lookup.
    let res = client
        .post(format!("{base}/commissions/{foreign_id}/invitations"))
        .json(&json!({ "seat": uuid::Uuid::now_v7(), "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("participant invite");
    common::assert_problem(res, 403, "forbidden").await;
}

// Floor — an anonymous caller cannot invite: 401.
#[tokio::test]
async fn anonymous_visitor_cannot_invite() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let seat = declare_seat(&signed_in, &base, id, root).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/invitations"))
        .json(&json!({ "seat": seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("anonymous invite");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Authorization binding — owning SOME commission grants no reach into another's
// offers. The caller owns commission A and holds the real seat id of an offer
// living in commission B; revoking through A's path is a 200 no-op and B's
// invitation stays pending (the lookup is commission-scoped in the store, so
// the cross-commission id resolves to nothing — never someone else's offer).
#[tokio::test]
async fn revoking_another_commissions_pending_offer_is_a_no_op() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    // Commission B holds the pending offer.
    let b = create_commission(&client, &base, &backend).await;
    let b_root = root_of(&backend, b).await;
    let b_seat = declare_seat(&client, &base, b, b_root).await;
    let res = client
        .post(format!("{base}/commissions/{b}/invitations"))
        .json(&json!({ "seat": b_seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("POST invite");
    assert_eq!(res.status(), 201, "the offer in B is issued");

    // Commission A is a different commission the caller also owns.
    let a = create_commission(&client, &base, &backend).await;
    let res = client
        .delete(format!("{base}/commissions/{a}/invitations"))
        .json(&json!({ "seat": b_seat, "user": "did:plc:invitee" }))
        .send()
        .await
        .expect("DELETE via the wrong commission");
    assert_eq!(res.status(), 200, "cross-commission revoke is a bare no-op");

    let invitee = backend
        .find_by_did(&Did::new("did:plc:invitee".to_string()))
        .await
        .expect("lookup")
        .expect("invitee was provisioned by the invite");
    let still_pending = backend
        .commission_store()
        .find_pending_seat_invitation(CommissionId::new(b), NodeId::new(b_seat), invitee.id)
        .await
        .expect("query");
    assert!(
        still_pending.is_some(),
        "B's offer survives a revoke attempted through another commission"
    );
}
