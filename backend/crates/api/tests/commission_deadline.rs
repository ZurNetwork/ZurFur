//! ZMVP-86 — the deadline axis, end to end over HTTP: a Participant sets or
//! clears the deadline and the manual Delayed "slipping" flag; the **system**
//! (the deadline sweeper) sets Late — the one place the system acts.
//!
//! Pins the acceptance criteria at the API surface (DESIGN/Commission, Status;
//! Engineer ruling 2026-07-05 on the ticket — Delayed is a MANUAL Participant
//! flag, the system sets ONLY Late, a standing Delayed upgrades to Late, no
//! derived threshold; conductor ruling E12 — the sweeper is a pure `sweep(now)`
//! over one unit of work; ruling E29 — one nullable column per axis):
//!
//! - **AC1** — the deadline lives on the commission envelope, nullable; a
//!   Participant sets or clears it (with `deadline_set`/`deadline_extended`
//!   changelog entries).
//! - **AC2** — a missed deadline sets Late on the deadline axis; a standing
//!   Delayed upgrades to Late.
//! - **AC3** — deadline-axis values are mutually exclusive (one cell); the
//!   axis composes freely with the direction axis.
//! - **AC4** — a commission with no deadline never receives deadline-axis
//!   statuses.
//! - **AC5** — the Late event lands in the changelog as a **system** entry
//!   (actor NULL), atomically with the status write.
//!
//! The sweep itself is exercised deterministically by calling
//! [`api::sweep_deadlines`] with an injected `now` — the same function the
//! composition root's interval task drives on the wall clock.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::{DateTime, Utc};
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, LifecycleStep},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect what was persisted (and drive the
/// sweeper against the same shared store). `did` is the identity `sign_in` will
/// authenticate as.
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

/// Creates a commission over HTTP as the signed-in caller (optionally with a
/// deadline) and returns its id — located by its (per-test unique) title,
/// because the route returns a bare `201` and
/// [`MemBackend::all_commissions`] lists in unspecified order.
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
    let title = body["title"].as_str().expect("body carries a title");
    let all = backend.all_commissions().await.expect("list commissions");
    *all.iter()
        .find(|c| c.title.as_str() == title)
        .expect("the created commission was persisted")
        .id
}

/// The persisted commission, rebuilt.
async fn stored(backend: &MemBackend, id: uuid::Uuid) -> Commission {
    backend
        .find_commission(CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists")
}

/// The persisted deadline status of `id`, as its stable wire token.
async fn stored_deadline_status(backend: &MemBackend, id: uuid::Uuid) -> Option<&'static str> {
    stored(backend, id)
        .await
        .deadline_status
        .map(|s| s.as_str())
}

/// PUT the deadline and return the response.
async fn put_deadline(
    client: &reqwest::Client,
    base: &str,
    id: uuid::Uuid,
    deadline: &str,
) -> reqwest::Response {
    client
        .put(format!("{base}/commissions/{id}/deadline"))
        .json(&json!({ "deadline": deadline }))
        .send()
        .await
        .expect("PUT deadline")
}

/// PUT the deadline-axis status (the manual slipping flag) and return the response.
async fn put_deadline_status(
    client: &reqwest::Client,
    base: &str,
    id: uuid::Uuid,
    status: &str,
) -> reqwest::Response {
    client
        .put(format!("{base}/commissions/{id}/status/deadline"))
        .json(&json!({ "status": status }))
        .send()
        .await
        .expect("PUT deadline status")
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

/// Runs one deterministic sweep at the injected instant — exactly what the
/// composition root's interval task does, minus the wall clock.
async fn sweep(backend: &MemBackend, now: DateTime<Utc>) -> usize {
    api::sweep_deadlines(&*backend.database(), now)
        .await
        .expect("sweep runs")
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), Some(past()));
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

/// Seeds a committed commission in the given lifecycle step with a long-missed
/// deadline, owned by a provisioned user — the arrangement the sweeper's
/// lifecycle scope tests need (no lifecycle-transition endpoint exists in this
/// lineage; the struct fields are public by design).
async fn seed_with_lifecycle(backend: &MemBackend, step: LifecycleStep) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:lifecycle-owner".to_string()))
        .await
        .expect("provision owner");
    let title = "Staged".parse::<CommissionTitle>().expect("valid title");
    let mut commission = Commission::create(title, owner.id, Utc::now(), Some(past()));
    commission.lifecycle_step = step;
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed staged commission");
    id
}

/// A deadline that is long past.
fn past() -> DateTime<Utc> {
    "2020-01-01T00:00:00Z".parse().expect("valid timestamp")
}

/// An instant comfortably after [`past`] (the sweeps' injected `now`).
fn after_past() -> DateTime<Utc> {
    "2020-06-01T00:00:00Z".parse().expect("valid timestamp")
}

// AC1 — a Participant sets the deadline on a commission born without one: the
// envelope field fills and a `deadline_set` entry (from: null) lands with the
// actor named.
#[tokio::test]
async fn a_participant_sets_the_deadline() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    let res = put_deadline(&client, &base, id, "2027-03-01T00:00:00Z").await;
    assert_eq!(res.status(), 204, "setting the deadline succeeds");

    let commission = stored(&backend, id).await;
    assert_eq!(
        commission.deadline,
        Some("2027-03-01T00:00:00Z".parse().unwrap()),
        "the envelope field holds the deadline"
    );

    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 2, "creation + deadline set");
    let set = &log[1];
    assert_eq!(set.kind.as_str(), "deadline_set");
    assert!(set.actor_id.is_some(), "an explicit set names its actor");
    assert!(set.payload["from"].is_null(), "born without a deadline");
    assert_eq!(set.payload["to"], "2027-03-01T00:00:00Z");
}

// AC1 — pushing an existing deadline later is an *extension* and records as
// `deadline_extended`; pulling it earlier is a plain re-set (`deadline_set`).
#[tokio::test]
async fn extending_the_deadline_emits_deadline_extended() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;

    let res = put_deadline(&client, &base, id, "2027-06-01T00:00:00Z").await;
    assert_eq!(res.status(), 204);
    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 2, "creation + extension");
    assert_eq!(log[1].kind.as_str(), "deadline_extended");
    assert_eq!(log[1].payload["from"], "2027-03-01T00:00:00Z");
    assert_eq!(log[1].payload["to"], "2027-06-01T00:00:00Z");

    // Pulling the deadline *earlier* is not an extension.
    let res = put_deadline(&client, &base, id, "2027-04-01T00:00:00Z").await;
    assert_eq!(res.status(), 204);
    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 3);
    assert_eq!(log[2].kind.as_str(), "deadline_set");
    assert_eq!(log[2].payload["from"], "2027-06-01T00:00:00Z");
    assert_eq!(log[2].payload["to"], "2027-04-01T00:00:00Z");
}

// AC1 (clear) + AC4 — clearing the deadline empties the envelope field AND
// wipes the deadline axis (a commission with no deadline never carries
// deadline-axis statuses); the clear records as `deadline_set` with `to: null`.
// Clearing an already-clear deadline is an idempotent no-op.
#[tokio::test]
async fn clearing_the_deadline_wipes_the_axis() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;

    // Flag it as slipping first, so the clear has an axis value to wipe.
    assert_eq!(
        put_deadline_status(&client, &base, id, "delayed")
            .await
            .status(),
        204
    );
    assert_eq!(stored_deadline_status(&backend, id).await, Some("delayed"));

    let res = client
        .delete(format!("{base}/commissions/{id}/deadline"))
        .send()
        .await
        .expect("DELETE deadline");
    assert_eq!(res.status(), 204, "clearing succeeds");

    let commission = stored(&backend, id).await;
    assert_eq!(commission.deadline, None, "the envelope field cleared");
    assert_eq!(
        commission.deadline_status, None,
        "no deadline ⇒ no deadline-axis status (AC4)"
    );

    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 3, "creation + delayed flag + deadline cleared");
    let clear = &log[2];
    assert_eq!(clear.kind.as_str(), "deadline_set");
    assert_eq!(clear.payload["from"], "2027-03-01T00:00:00Z");
    assert!(clear.payload["to"].is_null(), "a clear records to: null");

    // Clearing again changes nothing: 204, no noise entry.
    let res = client
        .delete(format!("{base}/commissions/{id}/deadline"))
        .send()
        .await
        .expect("DELETE deadline again");
    assert_eq!(res.status(), 204, "clearing twice is idempotent");
    assert_eq!(entries(&backend, id).await.len(), 3, "no no-op entry");
}

// Re-setting the deadline already held is the set-side no-op: 204, no entry.
#[tokio::test]
async fn re_setting_the_same_deadline_appends_no_entry() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;

    let res = put_deadline(&client, &base, id, "2027-03-01T00:00:00Z").await;
    assert_eq!(res.status(), 204, "a same-value set is idempotent");
    assert_eq!(
        entries(&backend, id).await.len(),
        1,
        "only the creation entry: nothing changed"
    );
}

// Engineer ruling 2026-07-05 — Delayed is a MANUAL Participant "slipping"
// flag: an explicit set, changelog-recorded with the actor (never a system
// entry); re-flagging is a no-op; the Participant clears their own flag.
#[tokio::test]
async fn a_participant_flags_the_commission_as_slipping() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;

    let res = put_deadline_status(&client, &base, id, "delayed").await;
    assert_eq!(res.status(), 204, "flagging as slipping succeeds");
    assert_eq!(stored_deadline_status(&backend, id).await, Some("delayed"));

    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 2, "creation + delayed flag");
    let delayed = &log[1];
    assert_eq!(delayed.kind.as_str(), "delayed");
    assert!(
        delayed.actor_id.is_some(),
        "Delayed is a manual Participant act, never a system entry"
    );
    assert_eq!(delayed.payload["to"], "delayed");

    // Re-flagging changes nothing: 204, no noise entry.
    let res = put_deadline_status(&client, &base, id, "delayed").await;
    assert_eq!(res.status(), 204, "re-flagging is idempotent");
    assert_eq!(entries(&backend, id).await.len(), 2);

    // The Participant clears their own flag; the clear is recorded.
    let res = client
        .delete(format!("{base}/commissions/{id}/status/deadline"))
        .send()
        .await
        .expect("DELETE deadline status");
    assert_eq!(res.status(), 204, "clearing the flag succeeds");
    assert_eq!(stored_deadline_status(&backend, id).await, None);
    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 3, "creation + flag + clear");
    assert_eq!(log[2].kind.as_str(), "delayed");
    assert_eq!(log[2].payload["from"], "delayed");
    assert!(log[2].payload["to"].is_null());

    // Clearing the already-clear flag is a no-op.
    let res = client
        .delete(format!("{base}/commissions/{id}/status/deadline"))
        .send()
        .await
        .expect("DELETE deadline status again");
    assert_eq!(res.status(), 204);
    assert_eq!(entries(&backend, id).await.len(), 3, "no no-op entry");
}

// Late is the system's word: a Participant cannot set it by hand (422), and a
// token outside the axis vocabulary is refused the same way.
#[tokio::test]
async fn late_cannot_be_set_by_hand() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;

    let res = put_deadline_status(&client, &base, id, "late").await;
    common::assert_problem(res, 422, "invalid_request").await;

    let res = put_deadline_status(&client, &base, id, "on_fire").await;
    common::assert_problem(res, 422, "invalid_request").await;

    let res = client
        .put(format!("{base}/commissions/{id}/status/deadline"))
        .json(&json!({ "wrong": "shape" }))
        .send()
        .await
        .expect("PUT malformed body");
    common::assert_problem(res, 422, "invalid_request").await;

    assert_eq!(
        stored_deadline_status(&backend, id).await,
        None,
        "nothing stored"
    );
    assert_eq!(
        entries(&backend, id).await.len(),
        1,
        "only the creation entry remains"
    );
}

// AC4 — a commission with no deadline never receives deadline-axis statuses:
// the manual flag is refused as a state conflict (409), and the sweeper never
// touches it.
#[tokio::test]
async fn a_deadlineless_commission_never_carries_deadline_statuses() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    let res = put_deadline_status(&client, &base, id, "delayed").await;
    common::assert_problem(res, 409, "no_deadline").await;

    assert_eq!(sweep(&backend, Utc::now()).await, 0, "nothing to sweep");
    assert_eq!(stored_deadline_status(&backend, id).await, None);
    assert_eq!(
        entries(&backend, id).await.len(),
        1,
        "only the creation entry — the sweeper wrote nothing"
    );
}

// AC2 + AC5 — the sweeper marks every missed deadline Late in one pass, and
// each Late mark lands in the changelog as a SYSTEM entry (actor NULL) whose
// payload names the missed deadline; a second sweep is a no-op (no duplicate
// entry — Late is already the system's standing word).
#[tokio::test]
async fn the_sweeper_marks_missed_deadlines_late() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let first = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "One", "deadline": "2020-01-01T00:00:00Z" }),
    )
    .await;
    let second = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Two", "deadline": "2020-02-01T00:00:00Z" }),
    )
    .await;
    let unbothered = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Future", "deadline": "2099-01-01T00:00:00Z" }),
    )
    .await;

    assert_eq!(
        sweep(&backend, after_past()).await,
        2,
        "both missed deadlines are marked in one sweep"
    );
    assert_eq!(stored_deadline_status(&backend, first).await, Some("late"));
    assert_eq!(stored_deadline_status(&backend, second).await, Some("late"));
    assert_eq!(
        stored_deadline_status(&backend, unbothered).await,
        None,
        "a future deadline is not late"
    );

    let log = entries(&backend, first).await;
    assert_eq!(log.len(), 2, "creation + the system Late entry");
    let late = &log[1];
    assert_eq!(late.kind.as_str(), "late");
    assert_eq!(late.actor_id, None, "the system entry carries no actor");
    assert_eq!(
        late.payload["deadline"], "2020-01-01T00:00:00Z",
        "the entry names the missed deadline (a sentence without joins)"
    );

    // Idempotent: an already-Late commission is not re-marked or re-logged.
    assert_eq!(
        sweep(&backend, after_past()).await,
        0,
        "nothing new to mark"
    );
    assert_eq!(
        entries(&backend, first).await.len(),
        2,
        "no duplicate entry"
    );
}

// AC2 — a standing (manual) Delayed upgrades to Late when the deadline passes;
// one cell, so Late REPLACES Delayed (mutual exclusivity by construction) and
// the system entry records what it upgraded from.
#[tokio::test]
async fn a_standing_delayed_upgrades_to_late() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2027-03-01T00:00:00Z" }),
    )
    .await;
    // Flag Delayed while the deadline is still ahead — allowed, because the
    // commission is not yet (derived) Late.
    assert_eq!(
        put_deadline_status(&client, &base, id, "delayed")
            .await
            .status(),
        204
    );
    // Pull the deadline into the past: the stored Delayed persists, and the
    // commission now DERIVES Late — the upgrade, computed at lookup.
    assert_eq!(
        put_deadline(&client, &base, id, "2020-01-01T00:00:00Z")
            .await
            .status(),
        204
    );
    assert_eq!(
        stored_deadline_status(&backend, id).await,
        Some("late"),
        "the passed deadline derives Late, superseding the standing Delayed"
    );

    // The sweep logs the Late transition, naming the flag it upgraded from.
    assert_eq!(sweep(&backend, after_past()).await, 1);
    let log = entries(&backend, id).await;
    assert_eq!(
        log.len(),
        4,
        "creation + delayed flag + deadline move + system Late"
    );
    assert_eq!(log[3].kind.as_str(), "late");
    assert_eq!(log[3].actor_id, None);
    assert_eq!(
        log[3].payload["from"], "delayed",
        "the upgrade records the standing flag it replaced"
    );
}

// Ruling E12 scope — the sweeper skips terminal lifecycles (Completed and
// Cancelled): a closed commission's missed deadline is history, not lateness.
// A Disputed commission is NOT terminal and is still swept (the dispute
// freeze — "deadlines freeze, Late pauses" — is the future Disputes epic).
#[tokio::test]
async fn the_sweeper_skips_terminal_lifecycles() {
    let (_base, backend) = spawn_app("did:plc:artist").await;
    let completed = seed_with_lifecycle(&backend, LifecycleStep::Completed).await;
    let cancelled = seed_with_lifecycle(&backend, LifecycleStep::Cancelled).await;
    let disputed = seed_with_lifecycle(&backend, LifecycleStep::Disputed).await;

    assert_eq!(
        sweep(&backend, after_past()).await,
        1,
        "only the disputed (non-terminal) commission is marked"
    );
    assert_eq!(stored_deadline_status(&backend, completed).await, None);
    assert_eq!(stored_deadline_status(&backend, cancelled).await, None);
    assert_eq!(
        stored_deadline_status(&backend, disputed).await,
        Some("late")
    );
    assert!(
        entries(&backend, completed).await.is_empty(),
        "nothing was appended to the closed commission's stream"
    );
}

// Extending a missed deadline into the future clears the system's Late mark
// (the deadline is no longer passed); missing the NEW deadline re-marks Late
// with a SECOND changelog entry — each miss is its own event.
#[tokio::test]
async fn extending_past_late_clears_it_and_a_second_miss_relogs() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2020-01-01T00:00:00Z" }),
    )
    .await;
    assert_eq!(sweep(&backend, after_past()).await, 1);
    assert_eq!(stored_deadline_status(&backend, id).await, Some("late"));

    // The Participant extends the deadline into the future: Late clears.
    let res = put_deadline(&client, &base, id, "2099-01-01T00:00:00Z").await;
    assert_eq!(res.status(), 204);
    assert_eq!(
        stored_deadline_status(&backend, id).await,
        None,
        "a deadline that hasn't passed can't be Late"
    );
    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 3, "creation + Late + extension");
    assert_eq!(log[2].kind.as_str(), "deadline_extended");

    // The new deadline is missed too: the deadline_extended entry re-arms the
    // log, so a NEW Late entry lands (each miss is its own event). Asserted on
    // the changelog — the sweep runs at an injected far-future instant, while the
    // derived state reads the real clock, so they'd disagree on this future date.
    let far_future: DateTime<Utc> = "2100-01-01T00:00:00Z".parse().unwrap();
    assert_eq!(sweep(&backend, far_future).await, 1);
    let log = entries(&backend, id).await;
    assert_eq!(log.len(), 4, "each miss is its own event");
    assert_eq!(log[3].kind.as_str(), "late");
    assert_eq!(log[3].payload["deadline"], "2099-01-01T00:00:00Z");
}

// While the commission is Late, the manual flag is out of reach: flagging
// Delayed over Late, or hand-clearing Late, is a state conflict (409) — the
// system's word is resolved through the deadline (extend or clear), never
// erased by hand.
#[tokio::test]
async fn the_systems_late_cannot_be_overridden_by_hand() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2020-01-01T00:00:00Z" }),
    )
    .await;
    assert_eq!(sweep(&backend, after_past()).await, 1);

    let res = put_deadline_status(&client, &base, id, "delayed").await;
    common::assert_problem(res, 409, "commission_late").await;

    let res = client
        .delete(format!("{base}/commissions/{id}/status/deadline"))
        .send()
        .await
        .expect("DELETE deadline status while late");
    common::assert_problem(res, 409, "commission_late").await;

    assert_eq!(
        stored_deadline_status(&backend, id).await,
        Some("late"),
        "Late stands"
    );

    // Clearing the DEADLINE, though, is the Participant's honest lever: no
    // deadline, nothing to be late against (AC4).
    let res = client
        .delete(format!("{base}/commissions/{id}/deadline"))
        .send()
        .await
        .expect("DELETE deadline");
    assert_eq!(res.status(), 204);
    assert_eq!(stored_deadline_status(&backend, id).await, None);
}

// AC3 — the axes compose freely: direction and deadline statuses coexist,
// neither write touches the other, and the sweeper moves ONLY the deadline
// axis — never the direction status, never the Lifecycle (no system event
// moves the Lifecycle — ZMVP-84's rule, re-asserted here).
#[tokio::test]
async fn the_axes_compose_freely() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(
        &client,
        &base,
        &backend,
        json!({ "title": "Ref", "deadline": "2020-01-01T00:00:00Z" }),
    )
    .await;

    // Direction + deadline axis together (Waiting for Approval + derived Late).
    let res = client
        .put(format!("{base}/commissions/{id}/status/direction"))
        .json(&json!({ "status": "waiting_for_approval" }))
        .send()
        .await
        .expect("PUT direction status");
    assert_eq!(res.status(), 204);

    let commission = stored(&backend, id).await;
    assert_eq!(
        commission.direction_status.map(|s| s.as_str()),
        Some("waiting_for_approval")
    );
    assert_eq!(
        commission.deadline_status.map(|s| s.as_str()),
        Some("late"),
        "the two axes hold values simultaneously — the passed deadline derives \
         Late on the deadline axis"
    );

    // The sweeper touches ONLY the deadline axis.
    let after_deadline: DateTime<Utc> = "2027-04-01T00:00:00Z".parse().unwrap();
    assert_eq!(sweep(&backend, after_deadline).await, 1);
    let commission = stored(&backend, id).await;
    assert_eq!(
        commission.direction_status.map(|s| s.as_str()),
        Some("waiting_for_approval"),
        "the sweep never moves the direction axis"
    );
    assert_eq!(commission.deadline_status.map(|s| s.as_str()), Some("late"));
    assert!(
        matches!(commission.lifecycle_step, LifecycleStep::Draft),
        "no system event moves the Lifecycle"
    );

    // And the deadline-axis write never touched the direction axis or deadline.
    let res = client
        .delete(format!("{base}/commissions/{id}/status/direction"))
        .send()
        .await
        .expect("DELETE direction status");
    assert_eq!(res.status(), 204);
    let commission = stored(&backend, id).await;
    assert_eq!(
        commission.deadline_status.map(|s| s.as_str()),
        Some("late"),
        "clearing the direction axis leaves the deadline axis alone"
    );
    assert!(commission.deadline.is_some());
}

// Closed door — a signed-in NON-participant acting on any deadline surface gets
// the **byte-identical** problem+json a nonexistent commission gets: a 404,
// never a 403 (no existence oracle), and nothing changes.
#[tokio::test]
async fn a_non_participant_gets_the_same_404_as_a_missing_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let missing = uuid::Uuid::now_v7();

    let res = put_deadline(&client, &base, foreign, "2027-01-01T00:00:00Z").await;
    assert_eq!(res.status(), 404, "hidden answers 404, never 403");
    let hidden_body: serde_json::Value = res.json().await.expect("problem body");
    let res = put_deadline(&client, &base, missing, "2027-01-01T00:00:00Z").await;
    assert_eq!(res.status(), 404);
    let missing_body: serde_json::Value = res.json().await.expect("problem body");
    assert_eq!(
        hidden_body, missing_body,
        "hidden and absent commissions are indistinguishable by construction"
    );
    assert_eq!(hidden_body["code"], "commission_not_found");

    let res = put_deadline_status(&client, &base, foreign, "delayed").await;
    common::assert_problem(res, 404, "commission_not_found").await;
    let res = client
        .delete(format!("{base}/commissions/{foreign}/deadline"))
        .send()
        .await
        .expect("DELETE foreign deadline");
    common::assert_problem(res, 404, "commission_not_found").await;
    let res = client
        .delete(format!("{base}/commissions/{foreign}/status/deadline"))
        .send()
        .await
        .expect("DELETE foreign deadline status");
    common::assert_problem(res, 404, "commission_not_found").await;

    let commission = stored(&backend, foreign).await;
    assert_eq!(
        commission.deadline,
        Some(past()),
        "the deadline never moved"
    );
    assert_eq!(
        commission.deadline_status.map(|s| s.as_str()),
        Some("late"),
        "the axis derives Late from the seeded past deadline — the non-participant \
         changed nothing (its stream below stays empty)"
    );
    assert!(
        entries(&backend, foreign).await.is_empty(),
        "nothing was appended to the hidden commission's stream"
    );
}

// The floor — anonymous callers are turned away with `401` on all four
// deadline surfaces.
#[tokio::test]
async fn anonymous_callers_are_turned_away() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend, json!({ "title": "Ref" })).await;

    let anon = client();
    let res = anon
        .put(format!("{base}/commissions/{id}/deadline"))
        .json(&json!({ "deadline": "2027-01-01T00:00:00Z" }))
        .send()
        .await
        .expect("anon PUT deadline");
    common::assert_problem(res, 401, "not_authenticated").await;
    let res = anon
        .delete(format!("{base}/commissions/{id}/deadline"))
        .send()
        .await
        .expect("anon DELETE deadline");
    common::assert_problem(res, 401, "not_authenticated").await;
    let res = anon
        .put(format!("{base}/commissions/{id}/status/deadline"))
        .json(&json!({ "status": "delayed" }))
        .send()
        .await
        .expect("anon PUT deadline status");
    common::assert_problem(res, 401, "not_authenticated").await;
    let res = anon
        .delete(format!("{base}/commissions/{id}/status/deadline"))
        .send()
        .await
        .expect("anon DELETE deadline status");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// A malformed deadline body ({}, or a null/garbage timestamp) is a 422 with
// nothing stored or appended.
#[tokio::test]
async fn a_malformed_deadline_body_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend, json!({ "title": "Ref" })).await;

    for body in [
        json!({}),
        json!({ "deadline": null }),
        json!({ "deadline": "yesterday-ish" }),
    ] {
        let res = client
            .put(format!("{base}/commissions/{id}/deadline"))
            .json(&body)
            .send()
            .await
            .expect("PUT malformed deadline");
        common::assert_problem(res, 422, "invalid_request").await;
    }

    let commission = stored(&backend, id).await;
    assert_eq!(commission.deadline, None, "nothing stored");
    assert_eq!(entries(&backend, id).await.len(), 1, "no entry appended");
}
