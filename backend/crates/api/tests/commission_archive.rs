//! The owner archives (and un-archives) a fact-bearing commission,
//! end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface:
//!
//! - **AC1** — the owner archives a commission: it is marked out of active
//!   views (`archived_at` set) but its record survives intact.
//! - **AC2** — an archived commission's facts remain intact and queryable: the
//!   changelog (its Total-tier record) still reads back for participants.
//! - **AC3** — a non-owner cannot archive: a non-participant gets the
//!   **byte-identical** 404 a missing commission gets (the closed door — never
//!   a 403 oracle).
//! - **Ruling** — un-archive exists as an explicit owner act; archive and
//!   un-archive are each changelog entries; facts are untouched both ways.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    did::Did,
    profile::Profile,
    user::User,
};
use serde_json::json;
use test_support::http::{client, serve, sign_in};

mod common;

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
    uuid::Uuid::from(all.last().expect("a commission was persisted").id)
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
        .provision(&Did::from("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = uuid::Uuid::from(commission.id);
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1 + AC2 — the owner archives: 204, the stored commission is marked archived
// but its record survives intact (title, owner), its changelog (the record every
// fact-to-come anchors beside) still reads back, and the act itself appended an
// `archived` entry naming the owner as actor with a payload that renders
// without joins.
#[tokio::test]
async fn the_owner_archives_and_the_record_survives() {
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
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/archive"))
        .send()
        .await
        .expect("POST archive");
    assert_eq!(res.status(), 204, "the owner archives the commission");

    let stored = backend
        .find_commission(domain::elements::commission::CommissionId::from(id))
        .await
        .expect("find")
        .expect("the record survives archiving");
    assert!(
        stored.archived_at.is_some(),
        "the commission is marked archived"
    );
    assert_eq!(
        stored.title.as_str(),
        "A ref sheet",
        "the record survives intact"
    );

    // AC2 — the record stays queryable for participants after archiving.
    let entries = read_changelog(&client, &base, id).await;
    assert_eq!(entries.len(), 2, "creation + the archive entry");
    assert_eq!(entries[1]["kind"], "archived");
    let me = backend
        .find_by_did(&Did::from("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");
    assert_eq!(
        entries[1]["actor_id"],
        json!(me.id.to_string()),
        "archiving is an owner act, never a system entry",
    );
    assert_eq!(
        entries[1]["payload"]["title"], "A ref sheet",
        "the payload renders a sentence without joins",
    );
}

// Engineer ruling 2026-07-05 — un-archive exists: the owner un-archives as an
// explicit act, the commission returns to active (archived_at cleared), and the
// act appends its own `unarchived` changelog entry after the `archived` one.
#[tokio::test]
async fn the_owner_unarchives_back_to_active() {
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
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/archive"))
        .send()
        .await
        .expect("POST archive");
    assert_eq!(res.status(), 204);

    let res = client
        .post(format!("{base}/commissions/{id}/unarchive"))
        .send()
        .await
        .expect("POST unarchive");
    assert_eq!(res.status(), 204, "the owner un-archives the commission");

    let stored = backend
        .find_commission(domain::elements::commission::CommissionId::from(id))
        .await
        .expect("find")
        .expect("still present");
    assert!(
        stored.archived_at.is_none(),
        "un-archiving returns the commission to active"
    );

    let entries = read_changelog(&client, &base, id).await;
    let kinds: Vec<&str> = entries
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["created", "archived", "unarchived"],
        "both directions are changelog entries, in act order",
    );
}

// Archiving an already-archived commission (and un-archiving an active one) is
// an idempotent no-op: 204, no state change, and — the changelog stays truthful
// — no duplicate entry (the clear-channel precedent: a record of nothing
// changing would be noise, not audit).
#[tokio::test]
async fn repeat_archive_and_unarchive_append_no_duplicate_entries() {
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
    let id = create_commission(&client, &base, &backend).await;

    // Un-archiving a never-archived commission: no-op, no entry.
    let res = client
        .post(format!("{base}/commissions/{id}/unarchive"))
        .send()
        .await
        .expect("POST unarchive on active");
    assert_eq!(
        res.status(),
        204,
        "un-archiving an active commission is a no-op"
    );

    // Archive twice: one entry.
    for _ in 0..2 {
        let res = client
            .post(format!("{base}/commissions/{id}/archive"))
            .send()
            .await
            .expect("POST archive");
        assert_eq!(res.status(), 204);
    }

    let entries = read_changelog(&client, &base, id).await;
    let kinds: Vec<&str> = entries
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["created", "archived"],
        "no-ops append nothing; the double archive left one entry",
    );
}

// AC3 (closed door) — a signed-in NON-participant archiving (or un-archiving)
// gets the **byte-identical** problem+json a nonexistent commission gets: a
// 404, never a 403, so the response is no existence oracle — and the foreign
// commission stays untouched.
#[tokio::test]
async fn a_non_owner_gets_the_same_404_as_a_missing_commission() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:outsider".to_string())).profile(
            Profile::new(
                Did::from("did:plc:outsider".to_string()),
                "artist.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "artist.bsky.social").await;
    let foreign = seed_foreign_commission(&backend).await;

    let res = client
        .post(format!("{base}/commissions/{foreign}/archive"))
        .send()
        .await
        .expect("POST foreign archive");
    assert_eq!(
        res.status(),
        404,
        "a hidden commission answers 404, never 403"
    );
    let hidden_body = res.text().await.expect("body");

    let missing = uuid::Uuid::now_v7();
    let res = client
        .post(format!("{base}/commissions/{missing}/archive"))
        .send()
        .await
        .expect("POST missing archive");
    assert_eq!(res.status(), 404);
    let missing_body = res.text().await.expect("body");
    assert_eq!(
        hidden_body, missing_body,
        "hidden and missing are indistinguishable (no existence oracle)",
    );

    let res = client
        .post(format!("{base}/commissions/{foreign}/unarchive"))
        .send()
        .await
        .expect("POST foreign unarchive");
    common::assert_problem(res, 404, "commission_not_found").await;

    let stored = backend
        .find_commission(domain::elements::commission::CommissionId::from(foreign))
        .await
        .expect("find")
        .expect("still present");
    assert!(stored.archived_at.is_none(), "the outsider changed nothing");
    assert!(
        backend
            .changelog_entries(domain::elements::commission::CommissionId::from(foreign))
            .await
            .expect("entries")
            .is_empty(),
        "no entry was appended by the refused acts",
    );
}

// The archive surface requires a session: an unauthenticated caller gets a 401
// in both directions, before any existence answer.
#[tokio::test]
async fn unauthenticated_archive_is_401() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:artist".to_string())).profile(Profile::new(
            Did::from("did:plc:artist".to_string()),
            "artist.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let signed_in = client();
    sign_in(&signed_in, &base, "artist.bsky.social").await;
    let id = create_commission(&signed_in, &base, &backend).await;

    let anonymous = client();
    let res = anonymous
        .post(format!("{base}/commissions/{id}/archive"))
        .send()
        .await
        .expect("POST archive unauthenticated");
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = anonymous
        .post(format!("{base}/commissions/{id}/unarchive"))
        .send()
        .await
        .expect("POST unarchive unauthenticated");
    common::assert_problem(res, 401, "not_authenticated").await;
}
