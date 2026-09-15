//! `GET /commissions` — the signed-in user's OWNED commissions,
//! owner-POV only:
//!
//! - the listing is owner-scoped (a commission owned by someone else never
//!   appears, even if the caller could otherwise reach it);
//! - archived commissions are excluded (an active-view listing);
//! - ordering is deterministic (by id — UUIDv7 sorts as creation order);
//! - an anonymous caller gets a `401` problem+json, same as `GET /me`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    did::Did,
    profile::Profile,
};

mod common;
use common::assert_problem;
use test_support::http::{client, serve, sign_in};

// Owner-scoped listing: a commission owned by a DIFFERENT user never appears,
// even though it exists in the same backend — the non-participant projection
// view (ZMVP-75) is a distinct, later surface this endpoint does not attempt.
// Also pins ordering: ascending by id (UUIDv7 sorts as creation order).
#[tokio::test]
async fn lists_only_commissions_the_caller_owns_in_deterministic_order() {
    let did = "did:plc:owner-lister";
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "lister.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "lister.bsky.social").await;
    let me = backend
        .find_by_did(&Did::from(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // Two commissions owned by the caller, created through the real write path.
    let mine_a = Commission::create(
        CommissionTitle::try_from("First".to_string()).expect("valid title"),
        me.id.clone(),
        Utc::now(),
        None,
    );
    backend
        .create_commission(&mine_a)
        .await
        .expect("seed commission A");
    let mine_b = Commission::create(
        CommissionTitle::try_from("Second".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&mine_b)
        .await
        .expect("seed commission B");

    // A commission owned by SOMEONE ELSE — must never appear on this list.
    let someone_else = backend
        .provision(&Did::from("did:plc:other-owner".to_string()))
        .await
        .expect("provision someone else");
    let theirs = Commission::create(
        CommissionTitle::try_from("Not mine".to_string()).expect("valid title"),
        someone_else.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&theirs)
        .await
        .expect("seed someone else's commission");

    let res = client
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    // Wrapped at the /api/v1 mint (contract R7): rows ride under "commissions".
    let rows = body["commissions"].as_array().expect("commissions array");
    assert_eq!(rows.len(), 2, "only the caller's own commissions: {body}");

    let ids: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !ids.contains(&theirs.id.to_string()),
        "someone else's commission must never appear"
    );

    let mut expected_order = vec![mine_a.id.to_string(), mine_b.id.to_string()];
    expected_order.sort();
    assert_eq!(
        ids, expected_order,
        "rows are ordered ascending by id (UUIDv7 = creation order)"
    );

    let first_row = rows
        .iter()
        .find(|row| row["id"] == mine_a.id.to_string())
        .expect("commission A is listed");
    assert_eq!(first_row["title"], "First");
    assert_eq!(first_row["lifecycle"], "draft");
    assert_eq!(first_row["visibility"], "private");
}

// Archived commissions are excluded — an active-view listing, per the
// documented listing-projection contract on `Commission::archived_at`
// (Deletion DD 3014657; ZMVP-68).
#[tokio::test]
async fn excludes_archived_commissions() {
    let did = "did:plc:archive-lister";
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "lister.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "lister.bsky.social").await;
    let me = backend
        .find_by_did(&Did::from(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    let active = Commission::create(
        CommissionTitle::try_from("Active".to_string()).expect("valid title"),
        me.id.clone(),
        Utc::now(),
        None,
    );
    backend
        .create_commission(&active)
        .await
        .expect("seed active commission");

    let mut archived = Commission::create(
        CommissionTitle::try_from("Archived".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    archived.archived_at = Some(Utc::now());
    backend
        .create_commission(&archived)
        .await
        .expect("seed archived commission");

    let res = client
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    // Wrapped at the /api/v1 mint (contract R7): rows ride under "commissions".
    let rows = body["commissions"].as_array().expect("commissions array");
    assert_eq!(rows.len(), 1, "the archived commission is excluded: {body}");
    assert_eq!(rows[0]["id"], active.id.to_string());
}

// An anonymous caller gets a 401 problem+json, exactly like `GET /me` — never a
// redirect, since the frontend calls this endpoint.
#[tokio::test]
async fn anonymous_caller_is_turned_away_with_401() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:anon-lister".to_string())).profile(
            Profile::new(
                Did::from("did:plc:anon-lister".to_string()),
                "lister.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);

    let res = client()
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_problem(res, 401, "not_authenticated").await;
}
