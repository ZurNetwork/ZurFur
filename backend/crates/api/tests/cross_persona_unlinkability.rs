//! Cross-persona unlinkability — *A User's alternate handles are never publicly correlated.*
//!
//! The invariant: a person who runs separate
//! personas does so as separate handles → separate Users → separate DIDs and
//! logins. **No public surface may join one handle's User/Account graph to
//! another's as belonging to the same person.** The separation holds *by
//! construction* (distinct DIDs, distinct logins), not by concealment — and the
//! only sanctioned correlation, opt-in User-Linking ("alts"), is post-MVP and
//! absent here.
//!
//! What this file guards — and what it deliberately does not:
//!
//! - **Forbidden, and tested here:** a public surface that groups a *person's*
//!   multiple Users/handles as one identity (the User-Linking relation). That
//!   concept does not exist in the MVP; these tests fail loudly if a change
//!   introduces one — e.g. an identity surface that names a caller's *other*
//!   personas, or a global user-enumeration endpoint.
//! - **Sanctioned, and NOT constrained here:** an Account-Profile roster of a
//!   *single* account's members/participants. Listing who is in
//!   one account does not assert that any two of them are the same human, so it is
//!   not cross-persona correlation. A future roster endpoint is a deliberate,
//!   designed surface — not something this guard should trip on.
//! - **Also sanctioned:** a caller reading *their own* accounts (`GET /accounts`).
//!   It is `401` to anonymous, takes no parameter naming a user, and
//!   names no User but the caller — so obtaining it requires already being that
//!   persona. The public User-Profile publishes strictly more than this (it
//!   lists account memberships by default), so a private
//!   self-listing cannot be the leak. What must stay true is behavioural, and is
//!   tested below: it never names a co-member — this guard is aimed at the
//!   leak itself, not merely at whether the route exists.
//!
//! Scope of the claim: *not correlated in-product, not
//! surfaced publicly by default* — not adversarial anonymity, since shared
//! infrastructure can still correlate for a determined observer.
//!
//! Harness: the same in-process fakes as the account/sign-in e2e tests — no
//! network, no database.
use domain::elements::{did::Did, profile::Profile};
use test_support::http::{client, serve, sign_in};

/// Persona A — the handle that signs in during these tests.
const ALICE_DID: &str = "did:plc:alice";
const ALICE_HANDLE: &str = "alice.bsky.social";

/// Persona B — a *separate* handle/User/DID that happens to share an account with
/// A. The same human could be behind both; the system has no way to know that and
/// must never imply it. Its handle is never served by any surface, so its presence
/// in a response body would itself be the leak.
const BOB_DID: &str = "did:plc:bob";
const BOB_HANDLE: &str = "bob.bsky.social";

/// The behavioral heart of cross-persona unlinkability: even when two personas share private state
/// (here, co-membership of one account), the public identity surface reflects
/// only the *caller's own* handle and never names the other persona. If a change
/// ever made `/me` (or its User-Profile successor) surface a caller's other
/// Users, this fails.
#[tokio::test]
async fn the_identity_surface_never_names_a_callers_other_persona() {
    let served = serve(
        test_support::runtime::mem(&Did::from(ALICE_DID.to_string()))
            .profile(Profile::new(Did::from(ALICE_DID.to_string()), ALICE_HANDLE)),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, ALICE_HANDLE).await;

    // Found an account as A, then seat B in it — the worst case for correlation:
    // A and B now share a row in the private account_members table. Granting via
    // the real seam provisions B as a separate User with a separate DID.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Shared Studio", "handle": "shared.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "A founds the account");
    let account_id = res.json::<serde_json::Value>().await.expect("json")["id"]
        .as_str()
        .expect("account id")
        .to_string();

    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": BOB_DID, "role": "member" }))
        .send()
        .await
        .expect("POST members");
    assert_eq!(
        res.status(),
        200,
        "A seats B as a member of the shared account"
    );

    // B is now a real, separate User sharing private state with A.
    assert!(
        backend
            .find_by_did(&Did::from(BOB_DID.to_string()))
            .await
            .expect("find_by_did")
            .is_some(),
        "B exists as a separate User",
    );

    // The public identity surface, read as A, shows A and only A. B — co-member,
    // possibly the same human — must not appear: no handle, no DID.
    let body = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .text()
        .await
        .expect("body");
    assert!(
        body.contains(ALICE_HANDLE),
        "/me reflects the caller's own handle, got: {body}"
    );
    assert!(
        !body.contains(BOB_DID) && !body.contains(BOB_HANDLE),
        "/me must not correlate the caller with a co-member's separate handle/DID, got: {body}"
    );

    // The own-accounts listing (ZMVP-157) is the other surface that could
    // correlate here: A and B share this account, so a listing that leaked its
    // roster would name B. It must return A's account WITHOUT naming the
    // co-member — this is the behavioural guard that replaced the old
    // route-absence assertion for `/accounts` (Engineer ruling 2026-07-25).
    let body = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts")
        .text()
        .await
        .expect("body");
    assert!(
        body.contains(&account_id),
        "/accounts lists the caller's own account, got: {body}"
    );
    assert!(
        !body.contains(BOB_DID) && !body.contains(BOB_HANDLE),
        "/accounts must not name a co-member's separate handle/DID, got: {body}"
    );
}

/// A signed-out viewer gets no identity surface at all — `/me` is an unauthenticated
/// 401 rather than leaking any handle or DID. Public presence on the platform never
/// starts from an enumerable identity read.
#[tokio::test]
async fn the_identity_surface_leaks_nothing_to_an_anonymous_viewer() {
    let served = serve(
        test_support::runtime::mem(&Did::from(ALICE_DID.to_string()))
            .profile(Profile::new(Did::from(ALICE_DID.to_string()), ALICE_HANDLE)),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let res = client()
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(
        res.status(),
        401,
        "an anonymous /me is unauthenticated, exposing no identity"
    );
}

/// Structural guard: there is no global user-enumeration surface. A list of all
/// Users (with the accounts each belongs to) is the classic way a person's
/// separate handles get correlated, so its *absence* is part of the invariant.
/// This fails the moment such a route is added — at which point cross-persona
/// unlinkability must be reconsidered, not silently regressed.
///
/// `/accounts` was removed from this list once it became a **caller-scoped**
/// read: it takes no
/// user-naming parameter, so it cannot enumerate anyone, and the property that
/// actually matters is tested behaviourally instead — see
/// [`the_identity_surface_never_names_a_callers_other_persona`], which asserts
/// the listing never names a co-member, and
/// [`the_own_accounts_listing_is_closed_to_anonymous_callers`] below. Those are
/// strictly stronger than an absence check: they fail on a leak, not merely on
/// a route existing.
#[tokio::test]
async fn there_is_no_global_user_enumeration_endpoint() {
    let served = serve(
        test_support::runtime::mem(&Did::from(ALICE_DID.to_string()))
            .profile(Profile::new(Did::from(ALICE_DID.to_string()), ALICE_HANDLE)),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let c = client();
    for path in ["/users", "/profiles", "/members"] {
        let res = c
            .get(format!("{base}{path}"))
            .send()
            .await
            .unwrap_or_else(|e| panic!("GET {path}: {e}"));
        let status = res.status();
        // 404 = the path is absent; 405 = the path exists for a write verb (e.g.
        // POST /accounts) but is not readable. Either way there is no GET surface
        // that enumerates Users across handles. A 2xx here would be the regression.
        assert!(
            status == 404 || status == 405,
            "GET {path} must not enumerate Users across handles; got {status}"
        );
    }
}

/// The half of the old `/accounts` absence assertion that still carries weight:
/// the own-accounts listing is **closed to anonymous callers**. A `2xx` here is
/// the real regression the structural guard was protecting against — it would
/// mean the listing had become a public surface, which is exactly what
/// cross-persona unlinkability forbids. `401` is the sanctioned answer (the route exists; you must be
/// someone to read it); anything in the 2xx range is a leak.
#[tokio::test]
async fn the_own_accounts_listing_is_closed_to_anonymous_callers() {
    let served = serve(
        test_support::runtime::mem(&Did::from(ALICE_DID.to_string()))
            .profile(Profile::new(Did::from(ALICE_DID.to_string()), ALICE_HANDLE)),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let res = client()
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(
        res.status(),
        401,
        "an anonymous /accounts is unauthenticated, exposing no membership graph"
    );
}
