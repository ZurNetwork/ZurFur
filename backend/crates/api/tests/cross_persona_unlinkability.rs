//! ZMVP-17 — *A User's alternate handles are never publicly correlated.*
//!
//! The surviving invariant after the 1DD "User-Profiles, the Handle Swap &
//! Content Maturity" (DESIGN, DECIDED 2026-06-22): a person who runs separate
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
//!   *single* account's members/participants (1DD decision 5). Listing who is in
//!   one account does not assert that any two of them are the same human, so it is
//!   not cross-persona correlation. A future roster endpoint is a deliberate,
//!   designed surface — not something this guard should trip on.
//!
//! Scope of the claim (1DD "Accepted tradeoff"): *not correlated in-product, not
//! surfaced publicly by default* — not adversarial anonymity, since shared
//! infrastructure can still correlate for a determined observer.
//!
//! Harness: the same in-process fakes as the account/sign-in e2e tests — no
//! network, no database.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Persona A — the handle that signs in during these tests.
const ALICE_DID: &str = "did:plc:alice";
const ALICE_HANDLE: &str = "alice.bsky.social";

/// Persona B — a *separate* handle/User/DID that happens to share an account with
/// A. The same human could be behind both; the system has no way to know that and
/// must never imply it. Its handle is never served by any surface, so its presence
/// in a response body would itself be the leak.
const BOB_DID: &str = "did:plc:bob";
const BOB_HANDLE: &str = "bob.bsky.social";

/// Boots the app with everything faked in-process, signing-in resolves to
/// [`ALICE_DID`]. Returns the base URL plus the user repo so a test can seat a
/// second persona directly. Mirrors `accounts.rs::spawn_app`.
async fn spawn_app() -> (String, MemBackend) {
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
            // ZMVP-49 config (unused by the mem minter in these tests).
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        // No route under test touches the database, so a lazy pool keeps the test
        // free of a container.
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(ALICE_DID.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(ALICE_DID.to_string()),
            handle: ALICE_HANDLE.to_string(),
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

/// A cookie-keeping client that does not auto-follow redirects, so each hop is
/// asserted on its own (same harness as the account/sign-in e2e).
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so the client's cookie jar carries a live session
/// for persona A.
async fn sign_in_as_alice(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("handle={ALICE_HANDLE}"))
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

/// The behavioral heart of ZMVP-17: even when two personas share private state
/// (here, co-membership of one account), the public identity surface reflects
/// only the *caller's own* handle and never names the other persona. If a change
/// ever made `/me` (or its User-Profile successor) surface a caller's other
/// Users, this fails.
#[tokio::test]
async fn the_identity_surface_never_names_a_callers_other_persona() {
    let (base, backend) = spawn_app().await;
    let client = client();
    sign_in_as_alice(&client, &base).await;

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
            .find_by_did(&Did::new(BOB_DID.to_string()))
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
}

/// A signed-out viewer gets no identity surface at all — `/me` is an unauthenticated
/// 401 rather than leaking any handle or DID. Public presence on the platform never
/// starts from an enumerable identity read.
#[tokio::test]
async fn the_identity_surface_leaks_nothing_to_an_anonymous_viewer() {
    let (base, _backend) = spawn_app().await;
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
/// This fails the moment such a route is added — at which point ZMVP-17 must be
/// reconsidered, not silently regressed.
#[tokio::test]
async fn there_is_no_global_user_enumeration_endpoint() {
    let (base, _backend) = spawn_app().await;
    let c = client();
    for path in ["/users", "/accounts", "/profiles", "/members"] {
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
