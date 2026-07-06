//! ZMVP-90 — a Participant adds Markup to a file entry, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface (DESIGN/Commission — "File
//! entries and Markup"; Engineer ruling E14 2026-07-05):
//!
//! - **AC1** — a Participant attaches a Markup (shape + coordinates + optional
//!   text) to a file entry; it lands as a `markup_added` changelog entry
//!   referencing the file entry's id, authored by the annotator.
//! - **AC2** — Markup is stored as raw structured data and returned
//!   untransformed (semantic fidelity — no coordinate transformation; rendering
//!   is the client's job). Validation on the way IN is strict (typed shapes,
//!   normalized 0–1 coordinates, capped text): the changelog is append-only, so
//!   malformed markup accepted today would be malformed forever.
//! - **AC3** — adding Markup changes NO Status: not the Lifecycle, not the
//!   direction axis, not the deadline axis (the always-explicit rule; the
//!   submission prompt is the explicit path).
//! - The target must be a VALIDATED existing file entry of THIS commission —
//!   an unknown id (or another commission's) is rejected, without becoming a
//!   cross-commission existence oracle.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, DeadlineStatus, DirectionStatus},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

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
            max_upload_bytes: 25 * 1024 * 1024,
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

/// Creates a commission over HTTP (with the given body) and returns its id,
/// resolved by its (unique) title — the route returns a bare `201`.
async fn create_commission_with(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
    body: serde_json::Value,
) -> uuid::Uuid {
    let title = body["title"].as_str().expect("a title").to_string();
    let res = client
        .post(format!("{base}/commissions"))
        .json(&body)
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
    create_commission_with(client, base, backend, json!({ "title": "A ref sheet" })).await
}

/// Uploads a small file entry into the commission and returns its file id.
async fn upload_file(client: &reqwest::Client, base: &str, commission: uuid::Uuid) -> String {
    let part = reqwest::multipart::Part::bytes(b"PNG-BYTES".to_vec())
        .file_name("sketch.png")
        .mime_str("image/png")
        .expect("valid mime");
    let form = reqwest::multipart::Form::new().part("file", part);
    let res = client
        .post(format!("{base}/commissions/{commission}/files"))
        .multipart(form)
        .send()
        .await
        .expect("POST file");
    assert_eq!(res.status(), 201, "the fixture upload succeeds");
    res.json::<serde_json::Value>().await.expect("file id")["id"]
        .as_str()
        .expect("id is a string")
        .to_string()
}

/// POSTs `body` as markup onto the file entry, returning the raw response.
async fn post_markup(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    file_id: &str,
    body: &serde_json::Value,
) -> reqwest::Response {
    client
        .post(format!(
            "{base}/commissions/{commission}/files/{file_id}/markup"
        ))
        .json(body)
        .send()
        .await
        .expect("POST markup")
}

/// Reads the changelog over HTTP as a JSON array.
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
        CommissionTitle::try_new("Not yours").expect("title"),
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

// AC1 — a Participant attaches a Markup (a circle, with a text comment) to a file
// entry: 201, and it lands as a `markup_added` changelog entry authored by the
// annotator, whose payload references the file entry's id and carries the markup.
#[tokio::test]
async fn a_participant_adds_markup_to_a_file_entry() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let file_id = upload_file(&client, &base, id).await;

    let markup = json!({
        "shape": { "circle": { "cx": 0.5, "cy": 0.25, "r": 0.125 } },
        "text": "make the tail fluffier here",
    });
    let res = post_markup(&client, &base, id, &file_id, &markup).await;
    assert_eq!(res.status(), 201, "adding markup returns 201");

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("provisioned");

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 3, "creation + file_added + markup_added");
    let entry = &entries[2];
    assert_eq!(entry["kind"], "markup_added");
    assert_eq!(
        entry["actor_id"],
        json!(*me.id),
        "the annotator is the actor"
    );
    assert_eq!(
        entry["payload"]["file_id"].as_str(),
        Some(file_id.as_str()),
        "the entry references the file entry it marks up"
    );
    assert_eq!(
        entry["payload"]["markup"], markup,
        "the markup is returned as submitted"
    );
}

// AC2 — every shape round-trips untransformed: what a client submits is exactly
// what the changelog serves back (rectangle and freehand here; circle in AC1).
// Rendering — and any coordinate math — is the client's job.
#[tokio::test]
async fn markup_round_trips_untransformed() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let file_id = upload_file(&client, &base, id).await;

    let rectangle = json!({
        "shape": { "rectangle": { "x": 0.25, "y": 0.25, "w": 0.5, "h": 0.375 } },
        "text": "crop to this region",
    });
    // No text — the optional comment stays absent, not nulled in.
    let freehand = json!({
        "shape": { "freehand": { "points": [[0.125, 0.5], [0.25, 0.625], [0.375, 0.5]] } },
    });

    for markup in [&rectangle, &freehand] {
        let res = post_markup(&client, &base, id, &file_id, markup).await;
        assert_eq!(res.status(), 201);
    }

    let entries = read_changelog(&client, &base, id).await;
    let markups: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| e["kind"] == "markup_added")
        .map(|e| &e["payload"]["markup"])
        .collect();
    assert_eq!(
        markups,
        [&rectangle, &freehand],
        "each markup comes back exactly as submitted — no transformation",
    );
}

// AC2 (the strict write gate) — malformed markup is 422 and appends NOTHING: an
// unknown shape, an unknown field, out-of-range coordinates (the space is
// normalized 0–1), degenerate extents, a malformed or trivial freehand stroke,
// blank or over-cap text, and a smuggled top-level field are all rejected.
// Append-only means malformed-forever, so the boundary is the only gate there
// will ever be.
#[tokio::test]
async fn malformed_markup_is_rejected_and_never_recorded() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let file_id = upload_file(&client, &base, id).await;

    let circle = |cx: f64, cy: f64, r: f64| json!({ "cx": cx, "cy": cy, "r": r });
    let rejected = [
        // An unknown shape kind.
        json!({ "shape": { "arrow": { "cx": 0.5, "cy": 0.5, "r": 0.1 } } }),
        // An unknown field inside a known shape.
        json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1, "color": "red" } } }),
        // Coordinates outside the normalized 0–1 space.
        json!({ "shape": { "circle": circle(-0.1, 0.5, 0.1) } }),
        json!({ "shape": { "circle": circle(1.5, 0.5, 0.1) } }),
        // Degenerate or oversized extents.
        json!({ "shape": { "circle": circle(0.5, 0.5, 0.0) } }),
        json!({ "shape": { "circle": circle(0.5, 0.5, 1.5) } }),
        json!({ "shape": { "rectangle": { "x": 0.1, "y": 0.1, "w": 0.0, "h": 0.5 } } }),
        // A missing coordinate.
        json!({ "shape": { "rectangle": { "x": 0.1, "y": 0.1, "w": 0.5 } } }),
        // Freehand: empty, a single point (not a stroke), a 3-tuple point, and an
        // out-of-range point.
        json!({ "shape": { "freehand": { "points": [] } } }),
        json!({ "shape": { "freehand": { "points": [[0.5, 0.5]] } } }),
        json!({ "shape": { "freehand": { "points": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]] } } }),
        json!({ "shape": { "freehand": { "points": [[0.1, 0.2], [0.3, 1.5]] } } }),
        // Text present but blank, and text over the cap.
        json!({ "shape": { "circle": circle(0.5, 0.5, 0.1) }, "text": "   " }),
        json!({ "shape": { "circle": circle(0.5, 0.5, 0.1) }, "text": "x".repeat(2001) }),
        // A smuggled top-level field (nothing rides along with a markup).
        json!({ "shape": { "circle": circle(0.5, 0.5, 0.1) }, "status": "waiting_for_approval" }),
        // No shape at all.
        json!({ "text": "just words" }),
    ];
    for body in &rejected {
        let res = post_markup(&client, &base, id, &file_id, body).await;
        common::assert_problem(res, 422, "invalid_request").await;
    }

    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(
        entries.len(),
        2,
        "creation + file_added only — no rejected markup was recorded",
    );
}

// The target must be a VALIDATED existing file entry of THIS commission: an
// absent file id is file_not_found, and a real file of a DIFFERENT commission is
// the same file_not_found — no cross-commission oracle. Nothing is appended.
#[tokio::test]
async fn markup_requires_an_existing_file_entry_of_this_commission() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let mine = create_commission_with(&client, &base, &backend, json!({ "title": "Mine" })).await;
    let other = create_commission_with(&client, &base, &backend, json!({ "title": "Other" })).await;
    let file_in_other = upload_file(&client, &base, other).await;

    let markup = json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1 } } });

    // A guessed, absent file id.
    let absent = uuid::Uuid::now_v7().to_string();
    let res = post_markup(&client, &base, mine, &absent, &markup).await;
    common::assert_problem(res, 404, "file_not_found").await;

    // A real file — of the other commission — pathed through mine.
    let res = post_markup(&client, &base, mine, &file_in_other, &markup).await;
    common::assert_problem(res, 404, "file_not_found").await;

    let entries = read_changelog(&client, &base, mine).await;
    assert_eq!(entries.len(), 1, "only the creation entry — nothing landed");
}

// The closed door — a non-participant cannot add markup to a hidden commission
// (the uniform commission_not_found 404, never a 403 oracle), and an anonymous
// caller is 401. Nothing is appended.
#[tokio::test]
async fn outsiders_cannot_add_markup() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;

    let markup = json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1 } } });
    let guessed = uuid::Uuid::now_v7().to_string();
    let res = post_markup(&client, &base, foreign, &guessed, &markup).await;
    common::assert_problem(res, 404, "commission_not_found").await;

    let anon = crate::client();
    let res = post_markup(&anon, &base, foreign, &guessed, &markup).await;
    common::assert_problem(res, 401, "not_authenticated").await;

    assert!(
        backend
            .changelog_entries(CommissionId::new(foreign))
            .await
            .expect("entries")
            .is_empty(),
        "nothing was appended to the hidden commission",
    );
}

// AC3 — adding Markup changes NO Status (the always-explicit rule; Engineer
// rulings 2026-07-01/2026-07-05). A commission holding a value on EVERY status
// axis — Lifecycle (draft), direction (waiting_for_input), deadline (Late, via a
// real sweep) — takes a markup; every axis is untouched, and the only new entry
// is the `markup_added` itself.
#[tokio::test]
async fn adding_markup_changes_no_status() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    // A deadline already in the past, so a sweep can put a real value on the
    // deadline axis.
    let past = Utc::now() - chrono::Duration::hours(2);
    let id = create_commission_with(
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

    let file_id = upload_file(&client, &base, id).await;

    // Snapshot every axis, and the changelog length, before the markup.
    let before = backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists");
    assert_eq!(
        before.direction_status,
        Some(DirectionStatus::WaitingForInput)
    );
    assert_eq!(before.deadline_status, Some(DeadlineStatus::Late));
    let log_before = read_changelog(&client, &base, id).await.len();

    let markup = json!({
        "shape": { "rectangle": { "x": 0.125, "y": 0.125, "w": 0.25, "h": 0.25 } },
        "text": "this part needs another pass",
    });
    let res = post_markup(&client, &base, id, &file_id, &markup).await;
    assert_eq!(res.status(), 201, "the markup itself lands");

    // Every status axis is exactly as it was.
    let after = backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists");
    assert_eq!(
        after.lifecycle_step.as_str(),
        before.lifecycle_step.as_str(),
        "markup never moves the Lifecycle"
    );
    assert_eq!(
        after.direction_status,
        Some(DirectionStatus::WaitingForInput),
        "markup never moves the direction axis"
    );
    assert_eq!(
        after.deadline_status,
        Some(DeadlineStatus::Late),
        "markup never moves the deadline axis"
    );
    assert_eq!(
        after.deadline, before.deadline,
        "the deadline itself is untouched"
    );

    // Exactly one new entry — the markup's own — and no status entry rode along.
    let log = read_changelog(&client, &base, id).await;
    assert_eq!(
        log.len(),
        log_before + 1,
        "the markup appends exactly its own entry, nothing else"
    );
    assert_eq!(log.last().expect("the new entry")["kind"], "markup_added");
    assert!(
        !log.iter()
            .skip(log_before)
            .any(|e| e["kind"] == "status_changed"),
        "no status_changed entry rode along with the markup"
    );
}
