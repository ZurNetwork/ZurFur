//! The browser session surface: the JSON whoami, the OAuth callback's
//! success and failure shapes, the sign-in failure shape, and the retirement of the
//! old HTML form route. Every dependency is faked in-process — the PDS
//! (`MemAuthenticator`/`MemProfileSource`), the user store (`MemBackend`), and the
//! session store (`MemoryStore`) — so these assert the *route* behavior, not the
//! storage tech (`PgSessionStore` is exercised in adapter-pg's own tests).
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adapter_mem::{MemAuthenticator, MemProfileSource};
use api::AppState;
use async_trait::async_trait;
use domain::{
    elements::{did::Did, handle::AtHandle, profile::Profile},
    ports::{
        AccountMismatch, Authenticator, BrowserBinding, BrowserMismatch, Database, ResolveError,
        SigninStarted, UnitOfWork,
    },
};
use test_support::http::{client, sign_in};
use test_support::runtime::MemRuntime;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{self, SessionStore};

mod common;

const DID: &str = "did:plc:sessionalice";

/// Which method of [`FailingAuthenticator`] errors, and with what — the
/// PDS-handshake failure points the callback and sign-in surfaces must map to
/// stable responses.
enum FailAt {
    /// `start` errors with this cause: the handle can't begin sign-in.
    Start(fn() -> anyhow::Error),
    /// `start` succeeds but `complete` errors: the code exchange fails at callback.
    Complete,
    /// `start` succeeds but the visitor signed in to a different account.
    WrongAccount,
    /// `start` succeeds but the callback came back to a browser the sign-in
    /// was not bound to, or after it expired.
    WrongBrowser,
}

/// An [`Authenticator`] that fails at a chosen point, standing in for a PDS that
/// rejects the handle or the code exchange. Lets the failure-shape tests drive the
/// error path the always-succeeding `MemAuthenticator` never can.
struct FailingAuthenticator {
    fail_at: FailAt,
}

/// What every stand-in authenticator's successful `start` answers.
fn started() -> SigninStarted {
    SigninStarted {
        authorization_url: "/signin-callback?code=test".to_string(),
        browser_binding: BrowserBinding::from(BINDING.to_string()),
        lifetime: Duration::from_secs(600),
    }
}

/// The browser token the stand-ins hand out.
const BINDING: &str = "test-browser-binding";

#[async_trait]
impl Authenticator for FailingAuthenticator {
    async fn start(&self, _handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        match self.fail_at {
            FailAt::Start(cause) => Err(cause()),
            FailAt::Complete | FailAt::WrongAccount | FailAt::WrongBrowser => Ok(started()),
        }
    }

    async fn complete(
        &self,
        _code: String,
        _state: Option<String>,
        _iss: Option<String>,
        _browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        match self.fail_at {
            FailAt::WrongAccount => Err(anyhow::Error::new(AccountMismatch)),
            FailAt::WrongBrowser => Err(anyhow::Error::new(BrowserMismatch)),
            FailAt::Start(_) | FailAt::Complete => Err(anyhow::anyhow!("code exchange failed")),
        }
    }
}

/// An [`Authenticator`] that records every handle `start` receives and every
/// browser token `complete` receives, then succeeds, so a test can see what the
/// route passed on, or that it passed nothing.
#[derive(Default)]
struct RecordingAuthenticator {
    started: Mutex<Vec<String>>,
    presented: Mutex<Vec<Option<String>>>,
}

impl RecordingAuthenticator {
    fn started(&self) -> Vec<String> {
        self.started.lock().expect("lock").clone()
    }

    fn presented(&self) -> Vec<Option<String>> {
        self.presented.lock().expect("lock").clone()
    }
}

#[async_trait]
impl Authenticator for RecordingAuthenticator {
    async fn start(&self, handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        self.started.lock().expect("lock").push(handle.to_string());
        Ok(started())
    }

    async fn complete(
        &self,
        _code: String,
        _state: Option<String>,
        _iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        let token = browser_binding.map(|binding| binding.as_ref().to_owned());
        self.presented.lock().expect("lock").push(token);
        Ok(Did::from(DID.to_string()))
    }
}

/// `POST /signin` with `handle` as the form field, URL-encoded.
async fn post_signin(base: &str, handle: &str) -> reqwest::Response {
    let body = format!("handle={}", urlencode(handle));
    client()
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("POST /signin")
}

/// Percent-encode every byte outside the unreserved set.
fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// A profile with a full complement of fields, so a JSON `/me` read can assert each.
fn alice_profile() -> Profile {
    Profile {
        did: Did::from(DID.to_string()),
        handle: "alice.bsky.social".to_string().into(),
        display_name: Some("Alice".to_string()),
        avatar_url: Some("https://pds.example/avatar/alice.jpg".to_string()),
    }
}

/// Boots the app with everything faked in-process, using the given authenticator and
/// profile source. Returns the base URL.
async fn serve(auth: Arc<dyn Authenticator>, source: Arc<MemProfileSource>) -> String {
    // `auth`/`source` are supplied per-call (a `FailingAuthenticator`, or a
    // profile source poisoned via `set_unreachable`), so the shared fixture is
    // built for its config/pool/stores and then overridden with them.
    let served = test_support::http::serve(
        test_support::runtime::mem(&Did::from(DID.to_string())),
        move |rt| {
            api::app(AppState {
                auth,
                profile_source: source,
                ..rt
            })
        },
    )
    .await;
    served.base_url
}

/// The default boot: an always-succeeding PDS that authenticates every visitor as
/// [`DID`] and serves [`alice_profile`].
async fn serve_happy() -> String {
    let auth = Arc::new(MemAuthenticator::new(Did::from(DID.to_string())));
    serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await
}

// --- GET /me --------------------------------------------------------------------

#[tokio::test]
async fn me_returns_json_identity_for_a_live_session() {
    let base = serve_happy().await;
    let client = client();
    sign_in(&client, &base, "alice.bsky.social").await;

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
    assert_eq!(body["displayName"], "Alice");
    assert_eq!(body["avatarUrl"], "https://pds.example/avatar/alice.jpg");
}

#[tokio::test]
async fn me_omits_the_profile_fields_when_the_pds_is_unreachable_and_uncached() {
    // The PDS is down and nothing is cached: /me still resolves the identity (the
    // DID) and simply omits the profile keys — absence is not an error.
    let source = MemProfileSource::new(alice_profile());
    source.set_unreachable();
    let auth = Arc::new(MemAuthenticator::new(Did::from(DID.to_string())));
    let base = serve(auth, Arc::new(source)).await;
    let client = client();
    sign_in(&client, &base, "alice.bsky.social").await;

    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "an unreachable PDS is not an error");
    let body: serde_json::Value = res.json().await.expect("body is JSON");
    assert_eq!(body["did"], DID, "the DID still proves who is signed in");
    // Minted R4 (2026-07-25): an absent optional OMITS its key — `null` is
    // never emitted; absence only ever means "not set".
    assert!(
        body.get("handle").is_none(),
        "handle key is omitted when unresolved, got {body}"
    );
    assert!(
        body.get("displayName").is_none(),
        "displayName key is omitted when unresolved, got {body}"
    );
    assert!(
        body.get("avatarUrl").is_none(),
        "avatarUrl key is omitted when unresolved, got {body}"
    );
}

#[tokio::test]
async fn concurrent_me_requests_for_one_did_share_one_fetch() {
    // Axum clones the runtime for every request: the lookups must still be one set.
    let source = Arc::new(MemProfileSource::new(alice_profile()));
    let auth = Arc::new(MemAuthenticator::new(Did::from(DID.to_string())));
    let base = serve(auth, source.clone()).await;
    let first_client = client();
    let second_client = client();
    sign_in(&first_client, &base, "alice.bsky.social").await;
    sign_in(&second_client, &base, "alice.bsky.social").await;
    source.answer_after(Duration::from_millis(500));
    let before = source.fetch_count();

    let (first, second) = tokio::join!(
        first_client.get(format!("{base}/me")).send(),
        second_client.get(format!("{base}/me")).send()
    );

    assert_eq!(first.expect("GET /me").status(), 200);
    assert_eq!(second.expect("GET /me").status(), 200);
    assert_eq!(
        source.fetch_count() - before,
        1,
        "two concurrent requests for one DID shared one PDS fetch"
    );
}

/// How long the web's session check waits for `GET /me` before it gives up.
const WEB_SESSION_CHECK_LIMIT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn me_answers_with_the_bare_did_within_the_wait_when_the_pds_never_answers() {
    // A cold cache and a PDS that never answers: /me stops waiting for the
    // profile and reports the DID alone, well inside the web's session check.
    let source = MemProfileSource::new(alice_profile());
    source.never_answer();
    let auth = Arc::new(MemAuthenticator::new(Did::from(DID.to_string())));
    let base = serve(auth, Arc::new(source)).await;
    let client = client();
    sign_in(&client, &base, "alice.bsky.social").await;

    let started = std::time::Instant::now();
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    let elapsed = started.elapsed();

    assert_eq!(res.status(), 200, "a silent PDS is not an error");
    let body: serde_json::Value = res.json().await.expect("body is JSON");
    assert_eq!(body["did"], DID, "the DID still proves who is signed in");
    assert!(
        body.get("handle").is_none(),
        "handle key is omitted when unresolved, got {body}"
    );
    assert!(
        elapsed >= application::user::PROFILE_WAIT,
        "/me waited for the profile first, answered after {elapsed:?}"
    );
    assert!(
        elapsed < WEB_SESSION_CHECK_LIMIT,
        "/me answered inside the web's session check, after {elapsed:?}"
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
    sign_in(&client, &base, "alice.bsky.social").await;

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
    let home = format!("{base}/");
    assert_eq!(
        res.headers()["location"],
        home.as_str(),
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
    let expected = format!("{base}/login?error=denied");
    assert_eq!(res.headers()["location"], expected.as_str());
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
    let expected = format!("{base}/login?error=invalid_callback");
    assert_eq!(res.headers()["location"], expected.as_str());
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
    let expected = format!("{base}/login?error=exchange_failed");
    assert_eq!(res.headers()["location"], expected.as_str());
}

// --- POST /signin ---------------------------------------------------------------

#[tokio::test]
async fn signin_failure_returns_an_invalid_request_problem() {
    // A handle the PDS won't begin sign-in for is a problem+json the frontend renders.
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::Start(|| anyhow::anyhow!("PDS rejected the handle")),
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
    let res = post_signin(&base, "alice.bsky.social").await;
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
    let expected = format!("{base}/login?error=invalid_callback");
    assert_eq!(res.headers()["location"], expected.as_str());
}

#[tokio::test]
async fn signin_refuses_urls_dids_and_at_prefixes_without_starting() {
    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(
        auth.clone(),
        Arc::new(MemProfileSource::new(alice_profile())),
    )
    .await;
    let not_handles = [
        "https://evil.example",
        "did:plc:z72i7hdynmk6r22z27h6tvur",
        "@alice.bsky.social",
        "",
    ];
    for typed in not_handles {
        let res = post_signin(&base, typed).await;
        common::assert_problem(res, 422, "invalid_request").await;
    }
    assert!(
        auth.started().is_empty(),
        "nothing that is not a handle reaches the authenticator"
    );
}

#[tokio::test]
async fn signin_trims_and_lowercases_what_the_visitor_typed() {
    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(
        auth.clone(),
        Arc::new(MemProfileSource::new(alice_profile())),
    )
    .await;
    let res = post_signin(&base, " \tAlice.Bsky.Social \n").await;
    assert_eq!(res.status(), 303, "a padded handle still signs in");
    let expected_handles = vec!["alice.bsky.social".to_string()];
    assert_eq!(auth.started(), expected_handles);
}

#[tokio::test]
async fn every_signin_failure_answers_one_identical_body() {
    /// The status, content type and raw body of a refused `POST /signin`.
    async fn refusal(base: &str, typed: &str) -> (u16, String, Vec<u8>) {
        let res = post_signin(base, typed).await;
        let status = res.status().as_u16();
        let content_type = res.headers()[reqwest::header::CONTENT_TYPE]
            .to_str()
            .expect("an ASCII content type")
            .to_string();
        let body = res.bytes().await.expect("body").to_vec();
        (status, content_type, body)
    }

    let causes: [fn() -> anyhow::Error; 5] = [
        || anyhow::Error::new(ResolveError::NotFound),
        || anyhow::Error::new(ResolveError::NotConfirmed),
        || anyhow::Error::new(ResolveError::Refused(anyhow::anyhow!("10.0.0.1"))),
        || anyhow::Error::new(ResolveError::Unavailable(anyhow::anyhow!("timed out"))),
        || anyhow::anyhow!("authorization endpoint refused"),
    ];
    let happy = serve_happy().await;
    let not_a_handle = refusal(&happy, "https://evil.example").await;
    assert_eq!(not_a_handle.0, 422);
    for cause in causes {
        let auth = Arc::new(FailingAuthenticator {
            fail_at: FailAt::Start(cause),
        });
        let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
        let failed = refusal(&base, "alice.bsky.social").await;
        assert_eq!(
            failed, not_a_handle,
            "every failure class answers the same bytes as a non-handle"
        );
    }
}

#[tokio::test]
async fn a_body_that_is_not_a_handle_form_answers_the_same_bytes_as_a_non_handle() {
    /// The status, content type and raw body `POST /signin` answers for `body`,
    /// sent as `content_type`.
    async fn answer(base: &str, content_type: &str, body: &'static str) -> (u16, String, Vec<u8>) {
        let res = client()
            .post(format!("{base}/signin"))
            .header("content-type", content_type)
            .body(body)
            .send()
            .await
            .expect("POST /signin");
        let status = res.status().as_u16();
        let content_type = res.headers()[reqwest::header::CONTENT_TYPE]
            .to_str()
            .expect("an ASCII content type")
            .to_string();
        let body = res.bytes().await.expect("body").to_vec();
        (status, content_type, body)
    }

    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(
        auth.clone(),
        Arc::new(MemProfileSource::new(alice_profile())),
    )
    .await;
    let form = "application/x-www-form-urlencoded";
    let not_a_handle = answer(&base, form, "handle=https%3A%2F%2Fevil.example").await;
    assert_eq!(not_a_handle.0, 422);
    let unreadable = [
        (form, ""),
        (form, "nickname=alice.bsky.social"),
        ("application/json", r#"{"handle":"alice.bsky.social"}"#),
        ("text/plain", "alice.bsky.social"),
    ];
    for (content_type, body) in unreadable {
        let refused = answer(&base, content_type, body).await;
        assert_eq!(
            refused, not_a_handle,
            "{content_type} {body:?} answers the same bytes as a non-handle"
        );
    }
    assert!(
        auth.started().is_empty(),
        "nothing the route cannot read reaches the authenticator"
    );
}

#[tokio::test]
async fn signin_callback_wrong_account_redirects_to_login_account_mismatch() {
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::WrongAccount,
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
    let res = client()
        .get(format!("{base}/signin-callback?code=test&state=s"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
    let expected = format!("{base}/login?error=account_mismatch");
    assert_eq!(res.headers()["location"], expected.as_str());
}

#[tokio::test]
async fn signin_callback_wrong_browser_redirects_to_login_browser_mismatch() {
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::WrongBrowser,
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;
    let res = client()
        .get(format!("{base}/signin-callback?code=test&state=s"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
    let expected = format!("{base}/login?error=browser_mismatch");
    assert_eq!(res.headers()["location"], expected.as_str());
}

// --- the browser-binding cookie -------------------------------------------------

/// Every `Set-Cookie` on `res` for the browser-binding cookie.
fn binding_cookies(res: &reqwest::Response) -> Vec<String> {
    res.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter(|cookie| cookie.starts_with("zurfur.signin="))
        .map(str::to_owned)
        .collect()
}

/// The `Set-Cookie` that clears the browser-binding cookie in dev.
const CLEARED: &str = "zurfur.signin=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";

#[tokio::test]
async fn signin_sets_the_browser_binding_cookie() {
    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;

    let res = post_signin(&base, "alice.bsky.social").await;

    assert_eq!(res.status(), 303);
    let expected = vec![format!(
        "zurfur.signin={BINDING}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600"
    )];
    assert_eq!(
        binding_cookies(&res),
        expected,
        "dev serves plain HTTP: no Secure"
    );
}

#[tokio::test]
async fn a_refused_signin_sets_no_cookie() {
    let auth = Arc::new(FailingAuthenticator {
        fail_at: FailAt::Start(|| anyhow::anyhow!("refused")),
    });
    let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;

    let res = post_signin(&base, "alice.bsky.social").await;

    assert_eq!(res.status(), 422);
    assert!(binding_cookies(&res).is_empty());
}

#[tokio::test]
async fn the_callback_hands_the_cookie_to_complete_and_clears_it() {
    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(
        auth.clone(),
        Arc::new(MemProfileSource::new(alice_profile())),
    )
    .await;

    let res = client()
        .get(format!("{base}/signin-callback?code=test&state=s"))
        .header(
            reqwest::header::COOKIE,
            "other=1; zurfur.signin=from-the-browser",
        )
        .send()
        .await
        .expect("GET /signin-callback");

    assert_eq!(res.status(), 303);
    let home = format!("{base}/");
    assert_eq!(res.headers()["location"], home.as_str());
    assert_eq!(binding_cookies(&res), vec![CLEARED.to_string()]);
    let expected_presented = vec![Some("from-the-browser".to_string())];
    assert_eq!(auth.presented(), expected_presented);
}

#[tokio::test]
async fn a_callback_without_the_cookie_presents_none() {
    let auth = Arc::new(RecordingAuthenticator::default());
    let base = serve(
        auth.clone(),
        Arc::new(MemProfileSource::new(alice_profile())),
    )
    .await;

    let res = client()
        .get(format!("{base}/signin-callback?code=test&state=s"))
        .header(reqwest::header::COOKIE, "zurfur.sid=whatever")
        .send()
        .await
        .expect("GET /signin-callback");

    assert_eq!(res.status(), 303);
    assert_eq!(auth.presented(), vec![None]);
}

/// Where the sign-in a [`BindingAuthenticator`] holds stands.
#[derive(Clone, Copy)]
enum Signin {
    /// Within its lifetime and unclaimed.
    Live,
    /// Past its lifetime.
    Expired,
    /// Another callback has claimed it and is still running.
    Claimed,
}

/// An [`Authenticator`] whose `complete` refuses the way the real one does: no
/// `state`, or a sign-in another callback claimed, fails generically; no token,
/// another token, or a sign-in past its lifetime is refused as
/// [`BrowserMismatch`]; the bound token on a live sign-in signs in as [`DID`].
struct BindingAuthenticator {
    signin: Signin,
}

#[async_trait]
impl Authenticator for BindingAuthenticator {
    async fn start(&self, _handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        Ok(started())
    }

    async fn complete(
        &self,
        _code: String,
        state: Option<String>,
        _iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        if state.is_none() {
            return Err(anyhow::anyhow!("unknown sign-in state"));
        }
        let presented = browser_binding.map(|binding| binding.as_ref().to_owned());
        let bound = presented.as_deref() == Some(BINDING);
        match self.signin {
            Signin::Claimed => Err(anyhow::anyhow!("sign-in already claimed")),
            Signin::Live if bound => Ok(Did::from(DID.to_string())),
            Signin::Live | Signin::Expired => Err(anyhow::Error::new(BrowserMismatch)),
        }
    }
}

/// `GET /signin-callback?{query}` from a browser sending `cookie`, if any.
async fn callback_with(base: &str, query: &str, cookie: Option<&str>) -> reqwest::Response {
    let request = client().get(format!("{base}/signin-callback?{query}"));
    let request = match cookie {
        Some(cookie) => request.header(reqwest::header::COOKIE, cookie),
        None => request,
    };
    request.send().await.expect("GET /signin-callback")
}

/// The browser's cookie holding the bound token.
fn bound_cookie() -> String {
    format!("zurfur.signin={BINDING}")
}

#[tokio::test]
async fn a_callback_that_matched_the_cookie_clears_it() {
    let signs_in: Arc<dyn Authenticator> = Arc::new(BindingAuthenticator {
        signin: Signin::Live,
    });
    let wrong_account: Arc<dyn Authenticator> = Arc::new(FailingAuthenticator {
        fail_at: FailAt::WrongAccount,
    });
    let cases = [
        (signs_in, "/"),
        (wrong_account, "/login?error=account_mismatch"),
    ];
    for (auth, location) in cases {
        let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;

        let res = callback_with(&base, "code=test&state=s", Some(&bound_cookie())).await;

        let expected = format!("{base}{location}");
        assert_eq!(res.headers()["location"], expected.as_str());
        assert_eq!(
            binding_cookies(&res),
            vec![CLEARED.to_string()],
            "{location} matched the cookie, so it clears it"
        );
    }
}

#[tokio::test]
async fn a_browser_refusal_leaves_the_cookie() {
    let wrong_cookie = "zurfur.signin=another-sign-ins-token".to_string();
    let cases = [
        ("no cookie", Signin::Live, None),
        ("a wrong cookie", Signin::Live, Some(wrong_cookie)),
        ("an expired sign-in", Signin::Expired, Some(bound_cookie())),
    ];
    for (case, signin, cookie) in cases {
        let auth = Arc::new(BindingAuthenticator { signin });
        let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;

        let res = callback_with(&base, "code=test&state=s", cookie.as_deref()).await;

        let expected = format!("{base}/login?error=browser_mismatch");
        assert_eq!(res.headers()["location"], expected.as_str(), "{case}");
        assert!(
            binding_cookies(&res).is_empty(),
            "{case} never matched, so the cookie is left: {:?}",
            binding_cookies(&res)
        );
    }
}

#[tokio::test]
async fn an_exchange_failure_leaves_the_cookie() {
    let claimed: Arc<dyn Authenticator> = Arc::new(BindingAuthenticator {
        signin: Signin::Claimed,
    });
    let no_state: Arc<dyn Authenticator> = Arc::new(BindingAuthenticator {
        signin: Signin::Live,
    });
    let generic: Arc<dyn Authenticator> = Arc::new(FailingAuthenticator {
        fail_at: FailAt::Complete,
    });
    let cases = [
        ("a claimed state", claimed, "code=test&state=s"),
        ("no state", no_state, "code=test"),
        ("a generic exchange failure", generic, "code=test&state=s"),
    ];
    for (case, auth, query) in cases {
        let base = serve(auth, Arc::new(MemProfileSource::new(alice_profile()))).await;

        let res = callback_with(&base, query, Some(&bound_cookie())).await;

        let expected = format!("{base}/login?error=exchange_failed");
        assert_eq!(res.headers()["location"], expected.as_str(), "{case}");
        assert!(
            binding_cookies(&res).is_empty(),
            "{case} proves no match, so the cookie is left: {:?}",
            binding_cookies(&res)
        );
    }
}

#[tokio::test]
async fn a_callback_the_authenticator_never_saw_leaves_the_cookie() {
    let cases = [
        ("error=access_denied", "/login?error=denied"),
        ("state=s", "/login?error=invalid_callback"),
    ];
    for (query, location) in cases {
        let auth = Arc::new(RecordingAuthenticator::default());
        let base = serve(
            auth.clone(),
            Arc::new(MemProfileSource::new(alice_profile())),
        )
        .await;

        let res = callback_with(&base, query, Some(&bound_cookie())).await;

        let expected = format!("{base}{location}");
        assert_eq!(res.headers()["location"], expected.as_str(), "{query}");
        assert!(
            binding_cookies(&res).is_empty(),
            "{query} left the cookie: {:?}",
            binding_cookies(&res)
        );
        assert!(
            auth.presented().is_empty(),
            "{query} never reached the authenticator"
        );
    }
}

// --- the two `500`s after the authenticator accepted the browser ----------------

/// A [`Database`] that cannot open a unit of work, so provisioning the User fails.
struct DownDatabase;

#[async_trait]
impl Database for DownDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        Err(anyhow::anyhow!("the private store is down"))
    }
}

/// A session store whose `delete` fails, so rotating the session id fails.
#[derive(Clone, Debug)]
struct DeleteFailingStore;

#[async_trait]
impl SessionStore for DeleteFailingStore {
    async fn save(&self, _record: &Record) -> session_store::Result<()> {
        Ok(())
    }

    async fn load(&self, _id: &Id) -> session_store::Result<Option<Record>> {
        Ok(None)
    }

    async fn delete(&self, _id: &Id) -> session_store::Result<()> {
        let down = session_store::Error::Backend("the session store is down".to_string());
        Err(down)
    }
}

/// Boots the app with a [`BindingAuthenticator`] on a live sign-in and its
/// sessions in `store`, in place of the shared fixture's `MemoryStore`. Returns
/// the base URL.
async fn serve_with_session_store(store: impl SessionStore + Clone) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let base_url = format!("http://{addr}");
    let MemRuntime { runtime, .. } = test_support::runtime::mem(&Did::from(DID.to_string()))
        .public_url(base_url.clone())
        .build();
    let auth = Arc::new(BindingAuthenticator {
        signin: Signin::Live,
    });
    let state = AppState { auth, ..runtime };
    let router = api::app(state).layer(http_cookies::session_layer(store, false));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    base_url
}

#[tokio::test]
async fn a_provisioning_failure_after_an_accepted_browser_clears_the_cookie() {
    let served = test_support::http::serve(
        test_support::runtime::mem(&Did::from(DID.to_string())),
        |rt| {
            let auth = Arc::new(BindingAuthenticator {
                signin: Signin::Live,
            });
            let state = AppState {
                auth,
                database: Arc::new(DownDatabase),
                ..rt
            };
            api::app(state)
        },
    )
    .await;

    let res = callback_with(&served.base_url, "code=test&state=s", Some(&bound_cookie())).await;

    assert_eq!(res.status(), 500);
    assert_eq!(
        binding_cookies(&res),
        vec![CLEARED.to_string()],
        "the authenticator accepted the browser, so the cookie is cleared"
    );
}

#[tokio::test]
async fn a_session_write_failure_after_an_accepted_browser_clears_the_cookie() {
    let base = serve_with_session_store(DeleteFailingStore).await;

    let res = callback_with(&base, "code=test&state=s", Some(&bound_cookie())).await;

    assert_eq!(res.status(), 500);
    assert_eq!(
        binding_cookies(&res),
        vec![CLEARED.to_string()],
        "the authenticator accepted the browser, so the cookie is cleared"
    );
}
