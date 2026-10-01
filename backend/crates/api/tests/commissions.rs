//! A User creates a commission and owns it, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the mem store-layer tests in
//! `adapter-mem` cover the persistence seam):
//!
//! - **AC1** — a signed-in User creates a commission by supplying a Title → `201`;
//! - **AC2/AC3** — the creating User is the owner and the commission is born in
//!   `Draft` with `Private` visibility (introspected off the backend, since the route
//!   returns a bare `201`);
//! - **AC4** — a User with **no Account** can create one (a user-scoped write; not
//!   gated on account membership: Users are first-class actors that need no Account);
//! - and the floors: an **anonymous** caller cannot create a commission (`401`), and
//!   a **blank title** is rejected (`422`, the `CommissionTitle` gate).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use domain::elements::{
    commission::{LifecycleStep, Visibility},
    did::Did,
    profile::Profile,
};
use serde_json::json;
use test_support::http::{client, serve, sign_in};

mod common;

// AC1/AC2/AC3 — a signed-in User creates a commission by Title, becomes its owner,
// and it is born in `Draft`. The route returns a bare `201`, so owner + lifecycle
// are read back off the shared backend.
#[tokio::test]
async fn signed_in_user_creates_a_commission_and_owns_it() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:artist".to_string())).profile(Profile::new(
            Did::from("did:plc:artist".to_string()),
            "artist.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "artist.bsky.social").await;

    let me = backend
        .find_by_did(&Did::from("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");

    let all = backend.all_commissions().await.expect("list commissions");
    assert_eq!(all.len(), 1, "exactly one commission was persisted");
    let commission = &all[0];
    assert_eq!(
        commission.title.as_str(),
        "A ref sheet",
        "the Title round-trips"
    );
    assert_eq!(commission.owner_id, me.id, "the creating User is the owner");
    assert!(
        matches!(commission.lifecycle_step, LifecycleStep::Draft),
        "a new commission is born in Draft",
    );
    assert!(
        matches!(commission.visibility, Visibility::Private),
        "a new commission is born Private (AC3)",
    );
}

// AC4 — a signed-in User holding ZERO accounts can still create a commission:
// creating one is a user-scoped write, not gated on account membership (ZMVP-47).
#[tokio::test]
async fn a_user_with_no_account_can_create_a_commission() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:newcomer".to_string())).profile(
            Profile::new(
                Did::from("did:plc:newcomer".to_string()),
                "artist.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "artist.bsky.social").await;
    // The signed-in user founds no account first — they hold none.

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "First commission" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(
        res.status(),
        201,
        "a zero-account User can create a commission (user-scoped write)",
    );
    assert_eq!(
        backend.all_commissions().await.expect("list").len(),
        1,
        "the commission was persisted",
    );
}

// The floor — an anonymous (signed-out) caller cannot create a commission: turned
// away at 401 `not_authenticated` (problem+json), and nothing is persisted.
#[tokio::test]
async fn anonymous_cannot_create_a_commission() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:nobody".to_string())).profile(Profile::new(
            Did::from("did:plc:nobody".to_string()),
            "artist.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);

    // No sign-in: the cookie jar carries no session.
    let res = client()
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "Should not persist" }))
        .send()
        .await
        .expect("POST /commissions");
    common::assert_problem(res, 401, "not_authenticated").await;

    assert!(
        backend.all_commissions().await.expect("list").is_empty(),
        "an unauthenticated create persists nothing",
    );
}

// Title validation — a blank (whitespace-only) title is rejected as `422`
// `invalid_request` (the `CommissionTitle` gate), and nothing is persisted.
#[tokio::test]
async fn a_blank_title_is_rejected() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:artist".to_string())).profile(Profile::new(
            Did::from("did:plc:artist".to_string()),
            "artist.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "artist.bsky.social").await;

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "   " }))
        .send()
        .await
        .expect("POST /commissions");
    common::assert_problem(res, 422, "invalid_request").await;

    assert!(
        backend.all_commissions().await.expect("list").is_empty(),
        "a blank-title create persists nothing",
    );
}
