//! The browser session surface (ZMVP-151): the JSON whoami, the OAuth callback's
//! success and failure shapes, the sign-in failure shape, and the retirement of the
//! old HTML form route. Every dependency is faked in-process — the PDS
//! (`MemAuthenticator`/`MemProfileSource`), the user store (`MemBackend`), and the
//! session store (`MemoryStore`) — so these assert the *route* behavior, not the
//! storage tech (`PgSessionStore` is exercised in adapter-pg's own tests).
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use async_trait::async_trait;
use domain::{
    elements::{did::Did, profile::Profile},
    ports::Authenticator,
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

const DID: &str = "did:plc:sessionalice";

/// Which method of [`FailingAuthenticator`] errors — the two PDS-handshake failure
/// points the callback and sign-in surfaces must map to stable responses.
enum FailAt {
    /// `start` errors: the handle can't begin sign-in (`POST /signin` failure).
    Start,
    /// `start` succeeds but `complete` errors: the code exchange fails at callback.
    Complete,
}

/// An [`Authenticator`] that fails at a chosen point, standing in for a PDS that
/// rejects the handle or the code exchange. Lets the failure-shape tests drive the
/// error path the always-succeeding `MemAuthenticator` never can.
struct FailingAuthenticator {
    fail_at: FailAt,
}

#[async_trait]
impl Authenticator for FailingAuthenticator {
    async fn start(&self, _handle: &str) -> anyhow::Result<String> {
        match self.fail_at {
            FailAt::Start => Err(anyhow::anyhow!("PDS rejected the handle")),
            FailAt::Complete => Ok("/signin-callback?code=test".to_string()),
        }
    }

    async fn complete(
        &self,
        _code: String,
        _state: Option<String>,
        _iss: Option<String>,
    ) -> anyhow::Result<Did> {
        Err(anyhow::anyhow!("code exchange failed"))
    }
}

/// A profile with a full complement of fields, so a JSON `/me` read can assert each.
fn alice_profile() -> Profile {
    Profile {
        did: Did::new(DID.to_string()),
        handle: "alice.bsky.social".to_string(),
        display_name: Some("Alice".to_string()),
        avatar_url: Some("https://pds.example/avatar/alice.jpg".to_string()),
    }
}

/// Boots the app with everything faked in-process, using the given authenticator and
/// profile source. Returns the base URL.
async fn serve(auth: Arc<dyn Authenticator>, source: Arc<MemProfileSource>) -> String {
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
        // No route here touches Postgres, so a lazy (never-connected) pool keeps the
        // test free of a container.
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth,
        users: backend.user_store(),
        profile_source: source,
        profile_cache: backend.profile_cache(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// The default boot: an always-succeeding PDS that authenticates every visitor as
/// [`DID`] and serves [`alice_profile`].
async fn serve_happy() -> String {
    let auth = Arc::new(MemAuthenticator::new(Did::new(DID.to_string())));
    serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await
}

/// A cookie-keeping client that does not auto-follow redirects, so each hop can be
/// asserted on its own.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Run the OAuth handshake, leaving the client holding a live session cookie.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=alice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin redirects to the PDS");
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback redirects on success");
}

// --- GET /me --------------------------------------------------------------------

#[tokio::test]
async fn me_returns_json_identity_for_a_live_session() {
    let base = serve_happy().await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "a live session gets a 200");
    assert_eq!(
        res.headers()[reqwest::header::CONTENT_TYPE],
        "application/json",
        "/me is JSON, not HTML",
    );
    let body: serde_json::Value = res.json().await.expect("body is JSON");
    assert_eq!(body["did"], DID, "did is always present");
    assert_eq!(body["handle"], "alice.bsky.social");
    assert_eq!(body["display_name"], "Alice");
    assert_eq!(body["avatar_url"], "https://pds.example/avatar/alice.jpg");
}

#[tokio::test]
async fn me_nulls_the_profile_fields_when_the_pds_is_unreachable_and_uncached() {
    // The PDS is down and nothing is cached: /me still resolves the identity (the
    // DID) and simply nulls the profile fields — absence is not an error.
    let source = MemProfileSource::new(alice_profile());
    source.set_unreachable();
    let auth = Arc::new(MemAuthenticator::new(Did::new(DID.to_string())));
    let base = serve(auth, Arc::new(source)).await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "an unreachable PDS is not an error");
    let body: serde_json::Value = res.json().await.expect("body is JSON");
    assert_eq!(body["did"], DID, "the DID still proves who is signed in");
    assert!(body["handle"].is_null(), "handle nulls out, got {body}");
    assert!(
        body["display_name"].is_null(),
        "display_name nulls out, got {body}"
    );
    assert!(
        body["avatar_url"].is_null(),
        "avatar_url nulls out, got {body}"
    );
}

#[tokio::test]
async fn me_returns_401_problem_for_an_anonymous_visitor() {
    // An anonymous /me is a 401 problem+json now, not a redirect: the frontend owns
    // the redirect to /login.
    let base = serve_happy().await;
    let res = client()
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// --- Cache-Control on the cookie surface (CWE-525, ZMVP-151) --------------------

#[tokio::test]
async fn me_401_carries_no_store_for_an_anonymous_visitor() {
    // Even the anonymous 401 problem response must forbid caching: the whole cookie
    // surface is no-store, not just the 200 body, so an intermediary can't stash a
    // stale authenticated/anonymous response.
    let base = serve_happy().await;
    let res = client()
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 401, "anonymous /me is a 401");
    assert_eq!(
        res.headers()[reqwest::header::CACHE_CONTROL],
        "no-store",
        "the cookie surface stamps Cache-Control: no-store",
    );
}

#[tokio::test]
async fn me_200_carries_no_store_for_a_live_session() {
    // The signed-in identity/PII JSON must not be cached by the browser or a shared
    // proxy (CWE-525).
    let base = serve_happy().await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "a live session gets a 200");
    assert_eq!(
        res.headers()[reqwest::header::CACHE_CONTROL],
        "no-store",
        "the authenticated /me body is no-store",
    );
}

#[tokio::test]
async fn health_is_not_scoped_into_the_no_store_layer() {
    // The public probe is deliberately left OUT of the cookie-surface cache layer —
    // over-scoping to the public routers was called out in review.
    let base = serve_happy().await;
    let res = client()
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("GET /health");
    assert!(
        res.headers().get(reqwest::header::CACHE_CONTROL).is_none(),
        "public /health carries no Cache-Control from the cookie-surface layer",
    );
}

// --- GET /signin-callback -------------------------------------------------------

#[tokio::test]
async fn signin_callback_success_redirects_to_root() {
    let base = serve_happy().await;
    let client = client();
    // Assert the first hop too: the test authenticator is stateless, so a broken
    // /signin would otherwise go unnoticed while the callback still succeeded.
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=alice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin redirects to the PDS");
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback redirects on success");
    assert_eq!(
        res.headers()["location"],
        "/",
        "a successful callback lands the visitor on the frontend root",
    );
}

#[tokio::test]
async fn signin_callback_user_denial_redirects_to_login_denied() {
    // A denial arrives with `error` and no `code`: stable code `denied`, no echo of
    // the PDS-supplied reason.
    let base = serve_happy().await;
    let res = client()
        .get(format!(
            "{base}/signin-callback?error=access_denied&error_description=the+user+said+no"
        ))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
    assert_eq!(res.headers()["location"], "/login?error=denied");
}

#[tokio::test]
async fn signin_callback_missing_code_redirects_to_login_invalid() {
    // No `code` and no `error`: an incomplete callback maps to `invalid_callback`.
    let base = serve_happy().await;
    let res = client()
        .get(format!("{base}/signin-callback?state=xyz"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
    assert_eq!(res.headers()["location"], "/login?error=invalid_callback");
}

#[tokio::test]
async fn signin_callback_exchange_failure_redirects_to_login_exchange_failed() {
    // The code is present but the PDS code-exchange fails: stable code `exchange_failed`.
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::Complete,
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
    let res = client()
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
    assert_eq!(res.headers()["location"], "/login?error=exchange_failed");
}

// --- POST /signin ---------------------------------------------------------------

#[tokio::test]
async fn signin_failure_returns_an_invalid_request_problem() {
    // A handle the PDS won't begin sign-in for is a problem+json the frontend renders.
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::Start,
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
    let res = client()
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=not-a-handle")
        .send()
        .await
        .expect("POST /signin");
    // A steady 422 with our terse code; the internal error is not echoed (the shape
    // is what the frontend branches on).
    common::assert_problem(res, 422, "invalid_request").await;
}

// --- Route surface --------------------------------------------------------------

#[tokio::test]
async fn the_html_form_route_is_gone_but_the_callback_remains() {
    let base = serve_happy().await;
    let c = client();

    // AC5: the old `GET /` sign-in form is retired — nothing serves it now.
    let res = c.get(format!("{base}/")).send().await.expect("GET /");
    assert_eq!(
        res.status(),
        404,
        "GET / no longer exists on the axum surface",
    );

    // …while the OAuth carve-out still reaches axum (an empty callback redirects to
    // /login rather than 404ing).
    let res = c
        .get(format!("{base}/signin-callback"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(
        res.status(),
        303,
        "/signin-callback still routes to the callback handler",
    );
    assert_eq!(res.headers()["location"], "/login?error=invalid_callback");
}
