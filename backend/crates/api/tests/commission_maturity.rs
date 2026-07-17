//! ZMVP-31 — a commission carries a required maturity rating (commissions-only
//! slice), end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface, per the Engineer ruling of
//! 2026-07-05 (the Maturity Vocabulary DD `29982722` supersedes the ticket's
//! pre-DD Safe/Questionable/Explicit text):
//!
//! - **the invariant** — a fresh commission starts **unrated** (`maturity`
//!   null): birth commissions are Private, so no rating is needed until the
//!   widening gate (ZMVP-74's job, which consumes this field);
//! - **the field** — the owner sets the posture via
//!   `PUT /commissions/{id}/maturity`: one of Safe / Suggestive / Nudity /
//!   Adult plus the orthogonal Graphic flag (omitted = not graphic);
//!   replace-only — there is deliberately no clear/DELETE route, so a rating,
//!   once given, can only change to another rating (a widened commission can
//!   never quietly become unrated);
//! - **server-side enforcement** — a value outside the enum is refused with a
//!   `422` (never stored, never defaulted), not merely hidden client-side;
//! - and the floors: an anonymous caller gets a `401`; a non-participant gets
//!   the **identical** uniform 404 an absent commission gets (the closed-door
//!   policy — never a 403 oracle).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    did::Did,
    maturity::{Maturity, MaturityRating},
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
        files: backend.file_store(),
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

/// The commission's persisted maturity posture, introspected off the backend.
async fn stored_maturity(backend: &MemBackend, id: uuid::Uuid) -> Option<Maturity> {
    backend
        .find_commission(domain::elements::commission::CommissionId::new(id))
        .await
        .expect("find commission")
        .expect("commission exists")
        .maturity
}

/// PUTs a maturity body and returns the response.
async fn put_maturity(
    client: &reqwest::Client,
    base: &str,
    id: uuid::Uuid,
    body: serde_json::Value,
) -> reqwest::Response {
    client
        .put(format!("{base}/commissions/{id}/maturity"))
        .json(&body)
        .send()
        .await
        .expect("PUT maturity")
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

// The invariant this ticket owns: a commission is born UNRATED — maturity is
// null at creation and stays null until the owner rates it. (Birth is Private;
// the rating becomes required only at widening, ZMVP-74.)
#[tokio::test]
async fn a_fresh_commission_starts_unrated() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    assert_eq!(
        stored_maturity(&backend, id).await,
        None,
        "a fresh commission carries no maturity value",
    );
}

// The at-creation shortcut: a caller may rate the commission in the create body
// itself, so a rating that's known up front lands in the SAME write — no second
// PUT round-trip. The posture (rating + graphic) persists exactly as supplied.
#[tokio::test]
async fn maturity_can_be_set_at_creation() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/commissions"))
        .json(
            &json!({ "title": "A ref sheet", "maturity": { "rating": "adult", "graphic": true } }),
        )
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a rated commission returns 201");

    let all = backend.all_commissions().await.expect("list commissions");
    let id = *all.last().expect("a commission was persisted").id;
    assert_eq!(
        stored_maturity(&backend, id).await,
        Some(Maturity {
            rating: MaturityRating::Adult,
            graphic: true,
        }),
        "the at-creation rating persists in one write — no follow-up PUT needed",
    );
}

// Server-side enforcement reaches the create path too: an out-of-vocabulary
// rating in the create body is a 422 BEFORE anything is written — the commission
// is not half-created and then rejected.
#[tokio::test]
async fn an_out_of_vocabulary_rating_at_creation_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet", "maturity": { "rating": "explicit" } }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 422, "a bad rating token at creation is a 422");

    assert!(
        backend
            .all_commissions()
            .await
            .expect("list commissions")
            .is_empty(),
        "a rejected creation persists no commission",
    );
}

// The field: the owner rates the commission — a 204, persisted as the chosen
// rating; the Graphic flag is optional and defaults to false when omitted.
#[tokio::test]
async fn the_owner_sets_a_maturity_rating() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = put_maturity(&client, &base, id, json!({ "rating": "nudity" })).await;
    assert_eq!(res.status(), 204, "setting a rating returns 204");

    assert_eq!(
        stored_maturity(&backend, id).await,
        Some(Maturity {
            rating: MaturityRating::Nudity,
            graphic: false,
        }),
        "the rating persists; graphic omitted = not graphic",
    );
}

// Every value of the DD's four-tier axis is accepted, the Graphic flag rides
// alongside any of them, and a second PUT replaces the posture (replace-only:
// there is no unrated state to go back to once rated — no clear route exists).
#[tokio::test]
async fn every_rating_is_accepted_and_a_new_put_replaces() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    for (token, rating) in [
        ("safe", MaturityRating::Safe),
        ("suggestive", MaturityRating::Suggestive),
        ("nudity", MaturityRating::Nudity),
        ("adult", MaturityRating::Adult),
    ] {
        for graphic in [true, false] {
            let res = put_maturity(
                &client,
                &base,
                id,
                json!({ "rating": token, "graphic": graphic }),
            )
            .await;
            assert_eq!(res.status(), 204, "rating {token:?} is in the vocabulary");
            assert_eq!(
                stored_maturity(&backend, id).await,
                Some(Maturity { rating, graphic }),
                "each PUT replaces the whole posture",
            );
        }
    }
}

// Server-side enforcement (AC): a value outside the enum is refused with a 422
// and its own problem code, and nothing is stored — including the superseded
// pre-DD vocabulary, case variants, and the derived *label* values (a rating
// is chosen as a rating, never smuggled in as its label).
#[tokio::test]
async fn out_of_vocabulary_ratings_are_refused_server_side() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    for bad in ["questionable", "explicit", "Safe", "porn", ""] {
        let res = put_maturity(&client, &base, id, json!({ "rating": bad })).await;
        common::assert_problem(res, 422, "unknown_maturity_rating").await;
    }
    // A malformed body (no rating at all) is a 422 too.
    let res = put_maturity(&client, &base, id, json!({ "graphic": true })).await;
    assert_eq!(res.status(), 422, "a body without a rating is refused");

    assert_eq!(
        stored_maturity(&backend, id).await,
        None,
        "no refused value was stored",
    );
}

// The floors: an anonymous caller cannot rate anything (401), and a
// non-participant gets the IDENTICAL uniform 404 an absent commission gets —
// the closed-door policy; never a 403, which would confirm existence.
#[tokio::test]
async fn anonymous_and_non_participant_callers_are_refused() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();

    let foreign = seed_foreign_commission(&backend).await;
    let body = json!({ "rating": "safe" });

    // Anonymous: 401 before anything else.
    let res = put_maturity(&client, &base, foreign, body.clone()).await;
    common::assert_problem(res, 401, "not_authenticated").await;

    // Signed in but not a participant: the uniform 404 …
    sign_in(&client, &base).await;
    let res = put_maturity(&client, &base, foreign, body.clone()).await;
    assert_eq!(res.status(), 404);
    let hidden: serde_json::Value = res.json().await.expect("problem body");

    // … byte-identical to a truly absent commission's 404.
    let res = put_maturity(&client, &base, uuid::Uuid::now_v7(), body).await;
    assert_eq!(res.status(), 404);
    let absent: serde_json::Value = res.json().await.expect("problem body");
    assert_eq!(
        hidden, absent,
        "hidden and absent commissions answer identically (no existence oracle)",
    );
    assert_eq!(hidden["code"], "commission_not_found");

    assert_eq!(
        stored_maturity(&backend, foreign).await,
        None,
        "the foreign commission is untouched",
    );
}
