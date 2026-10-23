//! The log canary: a whole signed-in visit on PostgreSQL, under the dev subscriber at `trace`
//! with `log` records bridged, writes no token, cookie, session id or key at any level, and no
//! DID or handle at info or above. The visit founds an Account, files bug reports, creates two
//! commissions with one title, opens the Den and one twin, and renames it. Alone in its binary:
//! it installs the global subscriber. The visitor's own thread stays out of the capture (this
//! crate's node says why).

use std::cell::Cell;
use std::sync::Arc;

use api::{AppState, Environment};
use async_trait::async_trait;
use domain::elements::{did::Did, handle::AtHandle, profile::Profile};
use domain::ports::{Authenticator, BrowserBinding, BrowserMismatch, SigninStarted};
use test_support::{http::client, log_capture::CapturedLog, pg_app};
use tracing_subscriber::{
    EnvFilter, filter::filter_fn, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

thread_local! {
    /// Set on the thread the visitor's client runs on: its records stay out of the capture.
    static VISITOR_THREAD: Cell<bool> = const { Cell::new(false) };
}

/// The signed-in user's DID and handle: debug lines may carry them, info and above never.
const USER_DID: &str = "did:plc:canaryuserdidq4f8a2c6";
const USER_HANDLE: &str = "canaryvisitor.test";

/// The OAuth callback's planted parameters, and the browser binding the sign-in hands out.
const CODE: &str = "canarycodeb81f4e2d9a";
const STATE: &str = "canarystate5c7a1e90d3";
const ISS: &str = "https://canaryissuer.example/x3k9";
const ISS_IN_A_QUERY: &str = "https%3A%2F%2Fcanaryissuer.example%2Fx3k9";
const BINDING: &str = "canarybinding2e6d9f4a7c";

/// The planted root key, and its base64 as the environment would carry it.
const ROOT_KEY: &str = "canary-root-key-7f3a9c1e5b2d8046";
const ROOT_KEY_BASE64: &str = "Y2FuYXJ5LXJvb3Qta2V5LTdmM2E5YzFlNWIyZDgwNDY=";

/// [`ROOT_KEY`] as the custody root the app is connected with.
fn canary_root() -> composition::RootSecret {
    let bytes: [u8; 32] = ROOT_KEY
        .as_bytes()
        .try_into()
        .expect("the planted root is 32 bytes");
    test_support::custody::root_of(bytes)
}

/// The Account the visit founds, so the Den lists something.
const ACCOUNT_HANDLE: &str = "canarystudio.zurfur.app";

/// The title the visit's two commissions share, so the Den numbers the newer one.
const TWIN_TITLE: &str = "Canary Twin Sketch";
/// The title the visit renames the newer twin to.
const RENAMED_TITLE: &str = "Canary Renamed Sketch";

/// What the visit's bug report carries: the reporter's words, page and browser are theirs.
const REPORT_DESCRIPTION: &str = "canarybugdescription4e8b1f";
const REPORT_PAGE: &str = "/canarybugpage7d2a9c";
const REPORT_BROWSER: &str = "canarybugbrowser1c5e";
/// The refused report's description, which a bidirectional override refuses.
const REFUSED_DESCRIPTION: &str = "canarybugrefused9a3d\u{202E}";
const REFUSED_CANARY: &str = "canarybugrefused9a3d";

/// An authenticator double that hands out [`BINDING`] and redirects back with [`CODE`],
/// [`STATE`] and [`ISS`], accepting only those, as the user [`USER_DID`].
struct CanaryAuthenticator;

#[async_trait]
impl Authenticator for CanaryAuthenticator {
    async fn start(&self, _handle: &AtHandle) -> anyhow::Result<SigninStarted> {
        let started = SigninStarted {
            authorization_url: format!(
                "/signin-callback?code={CODE}&state={STATE}&iss={ISS_IN_A_QUERY}"
            ),
            browser_binding: BrowserBinding::from(BINDING.to_owned()),
            lifetime: std::time::Duration::from_secs(600),
        };
        Ok(started)
    }

    async fn complete(
        &self,
        code: String,
        state: Option<String>,
        iss: Option<String>,
        browser_binding: Option<BrowserBinding>,
    ) -> anyhow::Result<Did> {
        let presented = browser_binding.ok_or(BrowserMismatch)?;
        let planted = code == CODE
            && state.as_deref() == Some(STATE)
            && iss.as_deref() == Some(ISS)
            && presented.as_ref() == BINDING;
        if !planted {
            return Err(BrowserMismatch.into());
        }
        Ok(Did::from(USER_DID.to_owned()))
    }
}

/// `text` without its ANSI color escapes.
fn without_colors(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_escape = false;
    for character in text.chars() {
        match (in_escape, character) {
            (false, '\u{1b}') => in_escape = true,
            (false, _) => plain.push(character),
            (true, 'm') => in_escape = false,
            (true, _) => {}
        }
    }
    plain
}

/// The live session id `response` sets, if it sets one.
fn session_id_set_by(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookie| cookie.split(';').next())
        .filter_map(|pair| pair.strip_prefix("zurfur.sid="))
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

/// The level of a formatted line: the first of its words that names one.
fn level_of(line: &str) -> Option<&str> {
    line.split_whitespace()
        .find(|word| ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].contains(word))
}

/// `POST /commissions` titled `title` as `browser`: its status and the new commission's id.
async fn create_commission(browser: &reqwest::Client, base: &str, title: &str) -> (u16, String) {
    let creation = serde_json::json!({ "title": title });
    let response = browser
        .post(format!("{base}/commissions"))
        .json(&creation)
        .send()
        .await
        .expect("POST /commissions");
    let status = response.status().as_u16();
    let created: serde_json::Value = response.json().await.expect("a JSON commission");
    let id = created["id"]
        .as_str()
        .expect("the commission's id")
        .to_owned();
    (status, id)
}

/// Whether some line of `text` carries every one of `parts`.
fn some_line_has(text: &str, parts: &[&str]) -> bool {
    text.lines()
        .any(|line| parts.iter().all(|part| line.contains(part)))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_whole_visit_logs_no_secret_at_any_level_and_no_identity_at_info() {
    let log = CapturedLog::default();
    let dev = api::logging::subscriber(&Environment::DEV, EnvFilter::new("trace"), log.clone());
    let server_side = filter_fn(|_| !VISITOR_THREAD.with(Cell::get));
    dev.with(server_side)
        .try_init()
        .expect("the canary's subscriber is this binary's only one");
    let served = pg_app::serve_configured(
        |runtime| {
            let state = AppState {
                auth: Arc::new(CanaryAuthenticator),
                ..runtime
            };
            api::app(state)
        },
        canary_root(),
        |_config| {},
    )
    .await;
    let profile = Profile::new(Did::from(USER_DID.to_owned()), USER_HANDLE);
    served.pds.register(profile);
    let base = &served.base_url;
    let browser = client();
    VISITOR_THREAD.with(|visitor| visitor.set(true));

    let started = browser
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("handle={USER_HANDLE}"))
        .send()
        .await
        .expect("POST /signin");
    let authorization_url = started
        .headers()
        .get("location")
        .and_then(|location| location.to_str().ok())
        .map(str::to_owned);
    let callback_path = authorization_url.expect("the sign-in redirects");
    let callback = browser
        .get(format!("{base}{callback_path}"))
        .send()
        .await
        .expect("GET /signin-callback");
    let session_id = session_id_set_by(&callback).expect("the callback sets the session");
    let session_number = session_id
        .parse::<tower_sessions::session::Id>()
        .map(|id| id.0.to_string())
        .expect("the session cookie holds a session id");
    let me = browser
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    let founding = serde_json::json!({ "name": "Canary Studio", "handle": ACCOUNT_HANDLE });
    let founded = browser
        .post(format!("{base}/accounts"))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts");
    let report = serde_json::json!({
        "description": REPORT_DESCRIPTION,
        "page": REPORT_PAGE,
        "browser": REPORT_BROWSER,
    });
    let filed = browser
        .post(format!("{base}/bug-reports"))
        .json(&report)
        .send()
        .await
        .expect("POST /bug-reports");
    let refused_report = serde_json::json!({ "description": REFUSED_DESCRIPTION });
    let refused = browser
        .post(format!("{base}/bug-reports"))
        .json(&refused_report)
        .send()
        .await
        .expect("POST /bug-reports");
    let root = browser
        .get(format!("{base}/den"))
        .send()
        .await
        .expect("GET /den");
    let accounts = browser
        .get(format!("{base}/den/accounts"))
        .send()
        .await
        .expect("GET /den/accounts");
    let accounts_listing: serde_json::Value = accounts.json().await.expect("a JSON listing");
    let (first_created, first_id) = create_commission(&browser, base, TWIN_TITLE).await;
    let (second_created, second_id) = create_commission(&browser, base, TWIN_TITLE).await;
    // The later UUIDv7 is the newer twin, whatever order the two creates landed in.
    let newer_twin = first_id.max(second_id);
    let commissions = browser
        .get(format!("{base}/den/commissions"))
        .send()
        .await
        .expect("GET /den/commissions");
    let twin = browser
        .get(format!("{base}/den/commissions/{newer_twin}"))
        .send()
        .await
        .expect("GET /den/commissions/{id}");
    let twin_status = twin.status();
    let twin_node: serde_json::Value = twin.json().await.expect("a JSON node");
    let renaming = serde_json::json!({ "title": RENAMED_TITLE });
    let renamed = browser
        .patch(format!("{base}/commissions/{newer_twin}/title"))
        .json(&renaming)
        .send()
        .await
        .expect("PATCH /commissions/{id}/title");
    let signed_out = browser
        .post(format!("{base}/logout"))
        .send()
        .await
        .expect("POST /logout");

    assert_eq!(started.status(), 303);
    assert_eq!(callback.status(), 303);
    assert_eq!(me.status(), 200);
    assert_eq!(founded.status(), 201);
    assert_eq!(filed.status(), 201);
    assert_eq!(refused.status(), 422);
    assert_eq!(root.status(), 200);
    let listed = accounts_listing["listing"]["entries"]
        .as_array()
        .is_some_and(|entries| !entries.is_empty());
    assert!(
        listed,
        "the accounts folder lists the Account: {accounts_listing}"
    );
    assert_eq!(first_created, 201);
    assert_eq!(second_created, 201);
    assert_eq!(commissions.status(), 200);
    assert_eq!(twin_status, 200);
    assert_eq!(
        twin_node["node"]["nameOrdinal"], 2,
        "the Den numbers the newer twin: {twin_node}"
    );
    assert_eq!(renamed.status(), 200);
    assert_eq!(signed_out.status(), 303);

    let text = without_colors(&log.text());
    // A reviewer may read the capture itself: LOG_CANARY_DUMP=<file> writes it there.
    if let Some(dump) = std::env::var_os("LOG_CANARY_DUMP") {
        std::fs::write(dump, &text).expect("the dump file is writable");
    }
    let controls: [(&str, &[&str]); 10] = [
        (
            "the callback's finish",
            &["request finished", "route=\"/signin-callback\""],
        ),
        (
            "the bug report's finish",
            &["request finished", "route=\"/bug-reports\""],
        ),
        (
            "the commission create's finish",
            &["request finished", "route=\"/commissions\"", "status=201"],
        ),
        (
            "the commission rename's finish",
            &[
                "request finished",
                "route=\"/commissions/{id}/title\"",
                "status=200",
            ],
        ),
        (
            "the rename use case's finish",
            &["application::commission::rename", "use case finished"],
        ),
        (
            "the Den root's finish",
            &["request finished", "route=\"/den\""],
        ),
        (
            "the Den listing's finish",
            &["request finished", "route=\"/den/{*path}\""],
        ),
        (
            "a use case's finish",
            &["use case finished", "outcome=\"ok\""],
        ),
        ("a store span's close", &["adapter_pg::", "close time.busy"]),
        (
            "the den check's answer",
            &["den check answered", "answer=\"opens\""],
        ),
    ];
    for (control, parts) in controls {
        assert!(some_line_has(&text, parts), "no line for {control}");
    }
    let secrets = [
        ("the OAuth code", CODE),
        ("the OAuth state", STATE),
        ("the issuer", ISS),
        ("the issuer as a query carries it", ISS_IN_A_QUERY),
        ("the browser binding", BINDING),
        ("the session id", session_id.as_str()),
        ("the session id as a number", session_number.as_str()),
        ("the root key", ROOT_KEY),
        ("the root key's base64", ROOT_KEY_BASE64),
        ("the bug report's description", REPORT_DESCRIPTION),
        ("the bug report's page", REPORT_PAGE),
        ("the bug report's browser", REPORT_BROWSER),
        ("the refused bug report's description", REFUSED_CANARY),
    ];
    for (secret, value) in secrets {
        let leaks: Vec<&str> = text.lines().filter(|line| line.contains(value)).collect();
        assert!(leaks.is_empty(), "the log carries {secret}: {leaks:#?}");
    }
    let identity_at_info: Vec<&str> = text
        .lines()
        .filter(|line| matches!(level_of(line), Some("INFO" | "WARN" | "ERROR")))
        .filter(|line| {
            [
                USER_DID,
                USER_HANDLE,
                ACCOUNT_HANDLE,
                "did:plc:",
                "did:web:",
            ]
            .iter()
            .any(|identity| line.contains(identity))
        })
        .collect();
    assert!(
        identity_at_info.is_empty(),
        "an info-or-above line names the user: {identity_at_info:#?}"
    );
}
