//! What the log says when two session cookie pairs reach the API. Alone in its
//! own test binary: tracing caches a callsite's interest from the first thread
//! that hits it, so a sibling test running without the capture could blind it.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemProfileSource};
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use test_support::http::{client, sign_in};
use test_support::log_capture::{self, CapturedLog};
use tower_sessions::session::Id;
use tracing_subscriber::EnvFilter;

const DID: &str = "did:plc:sessionalice";

/// A served app over the in-memory fixture world that signs every visitor in
/// as [`DID`]; returns its base URL.
async fn serve() -> String {
    let did = Did::from(DID.to_string());
    let profile = Profile::new(did.clone(), "alice.bsky.social");
    let served = test_support::http::serve(test_support::runtime::mem(&did), move |runtime| {
        let state = AppState {
            auth: Arc::new(MemAuthenticator::new(did)),
            profile_source: Arc::new(MemProfileSource::new(profile)),
            ..runtime
        };
        api::app(state)
    })
    .await;
    served.base_url
}

/// The live session id the sign-in callback set.
fn issued_session_id(callback: &reqwest::Response) -> String {
    callback
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| cookie.split(';').next()?.strip_prefix("zurfur.sid="))
        .expect("the callback sets the session cookie")
        .to_owned()
}

/// `GET {base}/me` sending `cookie` as its one `Cookie` header.
async fn me_with_cookie(base: &str, cookie: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!("{base}/me"))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("GET /me")
}

#[tokio::test]
async fn no_log_line_names_a_session_cookie_value_from_two_pairs() {
    let base = serve().await;
    let callback = sign_in(&client(), &base, "alice.bsky.social").await;
    let live = issued_session_id(&callback);
    let planted = Id(7).to_string();
    // Captured from here on, on the test's one thread, which also runs the server.
    let log = CapturedLog::default();
    let filter = EnvFilter::new("trace");
    let subscriber = api::logging::subscriber(&api::Environment::DEV, filter, log.clone());
    let _guard = log_capture::set_default(subscriber);
    let three_pairs = format!("zurfur.sid={live}; zurfur.sid=junkvalue; zurfur.sid={planted}");

    let res = me_with_cookie(&base, &three_pairs).await;
    // A control: one malformed pair draws tower-sessions' own warning, so the
    // capture is live and an empty log can't pass for a clean one.
    me_with_cookie(&base, "zurfur.sid=x").await;

    assert_eq!(res.status(), 401);
    let text = log.text();
    assert!(
        text.contains("malformed session id"),
        "the capture saw the control: {text}"
    );
    for value in [live.as_str(), "junkvalue", planted.as_str()] {
        assert!(!text.contains(value), "the log names {value:?}");
    }
}
