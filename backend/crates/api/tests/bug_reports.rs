//! `POST /bug-reports` through the real router, on the in-memory stores and on
//! PostgreSQL: the reporter is the session's, the API build is the server's,
//! absent items store nothing, a burst of parallel posts stores exactly the
//! limit, and no answer repeats what was sent.

use domain::elements::{
    bug_report::{BuildId, DisplaySettings, StoredBugReport},
    did::Did,
    profile::Profile,
};
use serde_json::{Value, json};
use sqlx::Connection as _;
use test_support::http::{Served, client, serve, sign_in};
use test_support::pg_app::{self, PgServed};
use test_support::runtime::{MemRuntimeBuilder, mem};

mod common;

/// The reporter every case signs in as: no fixture user, so a hard-coded one can't pass.
const REPORTER_DID: &str = "did:plc:bugreporterq7m4k2zx";
const REPORTER_HANDLE: &str = "bugreporter.test";

/// How many reports a burst posts at once: the number the security review sent.
const BURST: usize = 60;

/// The API build the build-id cases configure.
const API_BUILD: &str = "bugtest-7f3a";

/// The fixed answers the route gives; a test pins each one.
const UNREADABLE_BODY: &str = "The request body isn't a bug report this endpoint can read.";
const DESCRIPTION_REFUSED: &str = "The description must be 1 to 4000 characters, with no control \
                                   characters other than newlines and tabs.";
const PAGE_REFUSED: &str = "The page must be an app path starting with `/`, one line of at most \
                            2048 bytes, not starting with `//`, with no query, no fragment and no \
                            backslash.";
const PAGE_KIND_REFUSED: &str = "The page kind must be one line of at most 64 bytes.";
const WEB_VERSION_REFUSED: &str = "The web version must be one line of at most 64 bytes.";
const BROWSER_REFUSED: &str = "The browser must be one line of at most 512 bytes.";
const WINDOW_REFUSED: &str = "The window's width and height must each be 1 to 100000 pixels.";
const ERROR_STATUS_REFUSED: &str = "The error status must be an HTTP error status, 400 to 599.";

fn reporter_did() -> Did {
    Did::from(REPORTER_DID.to_owned())
}

fn reporter_profile() -> Profile {
    Profile::new(reporter_did(), REPORTER_HANDLE)
}

/// The in-memory runtime for the reporter.
fn reporter_runtime() -> MemRuntimeBuilder {
    mem(&reporter_did()).profile(reporter_profile())
}

/// `builder`'s app served in memory, and a client signed in as the reporter.
async fn signed_in(builder: MemRuntimeBuilder) -> (Served, reqwest::Client) {
    let served = serve(builder, api::app).await;
    let browser = client();
    sign_in(&browser, &served.base_url, REPORTER_HANDLE).await;
    (served, browser)
}

/// The app served on PostgreSQL with `build_id` configured, and a client signed in as the
/// reporter.
async fn signed_in_on_postgres(build_id: &str) -> (PgServed, reqwest::Client) {
    let build_id = BuildId::try_from(build_id).expect("a build id");
    let served = pg_app::serve_configured(api::app, |config| config.build_id = build_id).await;
    let browser = served.sign_in(reporter_profile()).await;
    (served, browser)
}

/// `POST {base}/bug-reports` with `body` as JSON.
async fn post_report(browser: &reqwest::Client, base: &str, body: &Value) -> reqwest::Response {
    browser
        .post(format!("{base}/bug-reports"))
        .json(body)
        .send()
        .await
        .expect("POST /bug-reports")
}

/// Every report the in-memory store holds, oldest first.
async fn stored_in_memory(served: &Served) -> Vec<StoredBugReport> {
    served
        .backend
        .bug_report_store()
        .list_since(None)
        .await
        .expect("the reports list")
}

/// Every report PostgreSQL holds, oldest first.
async fn stored_on_postgres(served: &PgServed) -> Vec<StoredBugReport> {
    served
        .runtime
        .bug_reports
        .list_since(None)
        .await
        .expect("the reports list")
}

/// The one stored report.
fn only(stored: Vec<StoredBugReport>) -> StoredBugReport {
    let [report] = <[StoredBugReport; 1]>::try_from(stored).expect("exactly one report is stored");
    report
}

/// An answer read whole: status, headers as text, body as text.
struct Answer {
    status: u16,
    headers: String,
    body: String,
}

impl Answer {
    async fn read(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let headers = format!("{:?}", response.headers());
        let body = response.text().await.expect("the body reads");
        Answer {
            status,
            headers,
            body,
        }
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).expect("the body is JSON")
    }

    /// Asserts a `422 invalid_request` whose detail is `detail`.
    fn assert_invalid(&self, detail: &str, case: &str) {
        let problem = self.json();
        assert_eq!(self.status, 422, "{case}: {}", self.body);
        assert_eq!(problem["code"], "invalid_request", "{case}");
        assert_eq!(problem["detail"], detail, "{case}");
    }

    /// Whether `text` appears anywhere in the answer.
    fn carries(&self, text: &str) -> bool {
        self.headers.contains(text) || self.body.contains(text)
    }
}

#[tokio::test]
async fn an_unknown_reporter_key_is_refused_and_stores_nothing() {
    let (served, browser) = signed_in(reporter_runtime()).await;

    for key in ["reporter", "reporterId"] {
        let mut body = json!({ "description": "Someone else filed this." });
        body[key] = json!("did:plc:someoneelsezz9");
        let answer = Answer::read(post_report(&browser, &served.base_url, &body).await).await;

        answer.assert_invalid(UNREADABLE_BODY, key);
    }
    let stored = stored_in_memory(&served).await;
    assert!(stored.is_empty(), "nothing is stored: {stored:?}");
}

#[tokio::test]
async fn the_stored_reporter_is_the_session_s_did() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "The board froze." });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_in_memory(&served).await);
    assert_eq!(report.reporter.to_string(), REPORTER_DID);
}

#[tokio::test]
async fn absent_items_store_null() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Only words." });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    assert_only_the_description(&only(stored_in_memory(&served).await));
}

#[tokio::test]
async fn absent_items_store_null_on_postgres() {
    let (served, browser) = signed_in_on_postgres("unknown").await;
    let body = json!({ "description": "Only words." });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_on_postgres(&served).await);
    assert_only_the_description(&report);
    assert_eq!(
        report.reporter.to_string(),
        REPORTER_DID,
        "the session's DID"
    );
}

/// Asserts `report` holds its description and nothing optional.
fn assert_only_the_description(report: &StoredBugReport) {
    assert_eq!(report.description.as_ref(), "Only words.");
    assert_eq!(report.page, None);
    assert_eq!(report.page_kind, None);
    assert_eq!(report.web_version, None);
    assert_eq!(report.browser, None);
    assert_eq!(report.window, None);
    assert_eq!(report.display, None);
    assert_eq!(report.error_status, None);
}

#[tokio::test]
async fn an_empty_display_stores_all_three_off() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Plain display.", "display": {} });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_in_memory(&served).await);
    assert_eq!(report.display, Some(DisplaySettings::default()));
}

#[tokio::test]
async fn an_empty_display_stores_all_three_off_on_postgres() {
    let (served, browser) = signed_in_on_postgres("unknown").await;
    let body = json!({ "description": "Plain display.", "display": {} });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_on_postgres(&served).await);
    assert_eq!(report.display, Some(DisplaySettings::default()));
}

#[tokio::test]
async fn every_sent_item_is_stored() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({
        "description": "The save button\tdid nothing.\nTwice.",
        "page": "/den/accounts",
        "pageKind": "den",
        "webVersion": "web-1.2.3",
        "browser": "Firefox/140.0",
        "window": { "width": 1280, "height": 720 },
        "display": { "reducedMotion": true, "moreContrast": false, "forcedColors": true },
        "errorStatus": 503,
    });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_in_memory(&served).await);
    let stored_text = |text: &Option<domain::elements::text::StoredText>| {
        text.as_ref().map(|text| text.as_ref().to_owned())
    };
    assert_eq!(
        report.description.as_ref(),
        "The save button\tdid nothing.\nTwice."
    );
    assert_eq!(stored_text(&report.page).as_deref(), Some("/den/accounts"));
    assert_eq!(stored_text(&report.page_kind).as_deref(), Some("den"));
    assert_eq!(
        stored_text(&report.web_version).as_deref(),
        Some("web-1.2.3")
    );
    assert_eq!(
        stored_text(&report.browser).as_deref(),
        Some("Firefox/140.0")
    );
    let window = report.window.expect("the window is stored");
    assert_eq!(u32::from(window.width), 1280);
    assert_eq!(u32::from(window.height), 720);
    let expected_display = DisplaySettings {
        reduced_motion: true,
        more_contrast: false,
        forced_colors: true,
    };
    assert_eq!(report.display, Some(expected_display));
    let status = report.error_status.expect("the error status is stored");
    assert_eq!(u16::from(status), 503);
}

#[tokio::test]
async fn api_version_is_the_api_s_own() {
    let build_id = BuildId::try_from(API_BUILD).expect("a build id");
    let (served, browser) = signed_in(reporter_runtime().build_id(build_id)).await;
    let body = json!({ "description": "Which build?", "webVersion": "web-9.9.9" });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_in_memory(&served).await);
    assert_eq!(report.api_version.as_ref(), API_BUILD);

    let claimed = json!({ "description": "Which build?", "apiVersion": "claimed-by-client" });
    let answer = Answer::read(post_report(&browser, &served.base_url, &claimed).await).await;
    answer.assert_invalid(UNREADABLE_BODY, "a posted apiVersion");
    assert_eq!(
        stored_in_memory(&served).await.len(),
        1,
        "nothing more stored"
    );
}

#[tokio::test]
async fn api_version_is_the_api_s_own_on_postgres() {
    let (served, browser) = signed_in_on_postgres(API_BUILD).await;
    let body = json!({ "description": "Which build?", "webVersion": "web-9.9.9" });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let report = only(stored_on_postgres(&served).await);
    assert_eq!(report.api_version.as_ref(), API_BUILD);
}

#[tokio::test]
async fn each_refused_item_gets_its_field_s_fixed_detail() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let too_long_description = "canarydesc".repeat(401);
    let cases = [
        (
            json!({ "description": "canarybell\u{7}x" }),
            DESCRIPTION_REFUSED,
            "canarybell",
        ),
        (
            json!({ "description": too_long_description }),
            DESCRIPTION_REFUSED,
            "canarydesc",
        ),
        (
            json!({ "description": "ok", "page": "canarypage/no-slash" }),
            PAGE_REFUSED,
            "canarypage",
        ),
        (
            json!({ "description": "ok", "pageKind": "canarykind\u{1}" }),
            PAGE_KIND_REFUSED,
            "canarykind",
        ),
        (
            json!({ "description": "ok", "webVersion": "canaryweb".repeat(8) }),
            WEB_VERSION_REFUSED,
            "canaryweb",
        ),
        (
            json!({ "description": "ok", "browser": "canarybrowser\u{202E}" }),
            BROWSER_REFUSED,
            "canarybrowser",
        ),
        (
            json!({ "description": "ok", "window": { "width": 0, "height": 77777 } }),
            WINDOW_REFUSED,
            "77777",
        ),
        (
            json!({ "description": "ok", "window": { "width": 1280, "height": 100001 } }),
            WINDOW_REFUSED,
            "100001",
        ),
        (
            json!({ "description": "ok", "errorStatus": 39999 }),
            ERROR_STATUS_REFUSED,
            "39999",
        ),
    ];

    for (body, detail, canary) in cases {
        let answer = Answer::read(post_report(&browser, &served.base_url, &body).await).await;

        answer.assert_invalid(detail, canary);
        assert!(
            !answer.carries(canary),
            "{canary} is echoed: {}",
            answer.body
        );
    }
    let stored = stored_in_memory(&served).await;
    assert!(stored.is_empty(), "nothing is stored: {stored:?}");
}

#[tokio::test]
async fn a_body_that_does_not_decode_gets_the_fixed_sentence() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let cases = [
        (
            json!({ "description": "ok", "canarykey8d2f": "canaryvalue5e1a" }),
            ["canarykey8d2f", "canaryvalue5e1a"],
        ),
        (
            json!({ "description": "ok", "window": { "width": "canarywidth3b7c" } }),
            ["canarywidth3b7c", "width"],
        ),
        (
            json!({ "description": 4242, "pageKind": "canarytype6c0e" }),
            ["canarytype6c0e", "4242"],
        ),
    ];

    for (body, canaries) in cases {
        let answer = Answer::read(post_report(&browser, &served.base_url, &body).await).await;

        answer.assert_invalid(UNREADABLE_BODY, canaries[0]);
        for canary in canaries {
            assert!(
                !answer.carries(canary),
                "{canary} is echoed: {}",
                answer.body
            );
        }
    }
}

#[tokio::test]
async fn the_11th_report_within_the_hour_is_refused_with_429() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Again." });

    for nth in 1..=10 {
        let filed = post_report(&browser, &served.base_url, &body).await;
        assert_eq!(filed.status(), 201, "report {nth} is accepted");
    }
    let eleventh = post_report(&browser, &served.base_url, &body).await;

    common::assert_problem(eleventh, 429, "rate_limited").await;
    assert_eq!(stored_in_memory(&served).await.len(), 10);
}

/// Posts [`BURST`] reports at once and returns how many answered 201 and how many 429,
/// asserting every answer is one of the two.
async fn post_a_burst(browser: &reqwest::Client, base: &str) -> (usize, usize) {
    let mut posts = tokio::task::JoinSet::new();
    for _ in 0..BURST {
        let browser = browser.clone();
        let base = base.to_owned();
        posts.spawn(async move {
            let body = json!({ "description": "Burst." });
            post_report(&browser, &base, &body).await.status().as_u16()
        });
    }
    let statuses = posts.join_all().await;
    let created = statuses.iter().filter(|&&status| status == 201).count();
    let limited = statuses.iter().filter(|&&status| status == 429).count();
    assert_eq!(
        created + limited,
        BURST,
        "every answer is 201 or 429: {statuses:?}"
    );
    (created, limited)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_parallel_reports_stores_exactly_the_limit() {
    let (served, browser) = signed_in(reporter_runtime()).await;

    let (created, limited) = post_a_burst(&browser, &served.base_url).await;

    assert_eq!((created, limited), (10, BURST - 10));
    assert_eq!(stored_in_memory(&served).await.len(), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_parallel_reports_stores_exactly_the_limit_on_postgres() {
    let (served, browser) = signed_in_on_postgres("unknown").await;

    let (created, limited) = post_a_burst(&browser, &served.base_url).await;

    assert_eq!((created, limited), (10, BURST - 10));
    assert_eq!(stored_on_postgres(&served).await.len(), 10);
}

/// Waits until at least `waiters` backends of `watcher`'s database wait on a lock.
async fn wait_for_lock_waiters(watcher: &mut sqlx::PgConnection, waiters: i64) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
             WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(&mut *watcher)
        .await
        .expect("pg_stat_activity reads");
        if waiting >= waiters {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "only {waiting} backends wait on a lock"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Nine reports filed, then a burst held at the reporter's `users` row until several filing
/// units queue on it: without the rate limit's lock they would all count nine and file.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_racing_for_the_last_report_stores_exactly_the_limit_on_postgres() {
    let (served, browser) = signed_in_on_postgres("unknown").await;
    let body = json!({ "description": "Before the burst." });
    for nth in 1..=9 {
        let filed = post_report(&browser, &served.base_url, &body).await;
        assert_eq!(filed.status(), 201, "report {nth} is accepted");
    }
    let options = served.runtime.pool.connect_options();
    let mut holder = sqlx::PgConnection::connect_with(&options)
        .await
        .expect("the holder connects");
    let mut watcher = sqlx::PgConnection::connect_with(&options)
        .await
        .expect("the watcher connects");
    // FOR UPDATE blocks the rate limit's FOR NO KEY UPDATE, and an insert's
    // foreign-key check too, but not a bare count.
    let mut holding = holder.begin().await.expect("the holder begins");
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(REPORTER_DID)
        .execute(&mut *holding)
        .await
        .expect("the holder locks the reporter");

    let burst_browser = browser.clone();
    let burst_base = served.base_url.clone();
    let burst = tokio::spawn(async move { post_a_burst(&burst_browser, &burst_base).await });
    wait_for_lock_waiters(&mut watcher, 2).await;
    holding.rollback().await.expect("the holder lets go");
    let (created, limited) = burst.await.expect("the burst ran");

    assert_eq!((created, limited), (1, BURST - 1));
    assert_eq!(stored_on_postgres(&served).await.len(), 10);
}

#[tokio::test]
async fn no_route_reads_reports() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Read me back?" });
    let filed: Value = post_report(&browser, &served.base_url, &body)
        .await
        .json()
        .await
        .expect("the receipt is JSON");
    let reference = filed["reference"].as_str().expect("a reference");
    let base = &served.base_url;

    let list = browser
        .get(format!("{base}/bug-reports"))
        .send()
        .await
        .expect("GET /bug-reports");
    assert_eq!(list.status(), 405, "the collection has no GET");

    let reads = [
        format!("{base}/bug-reports/{reference}"),
        format!("{base}/bug-reports?since=2026-01-01T00:00:00Z"),
    ];
    for read in reads {
        let answer = browser.get(&read).send().await.expect("GET a read");
        let status = answer.status().as_u16();
        assert!(
            [404, 405].contains(&status),
            "{read} answers {status}, not a read"
        );
    }
}

#[tokio::test]
async fn the_answer_carries_only_reference_and_received_at() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Receipt, please." });

    let answer = Answer::read(post_report(&browser, &served.base_url, &body).await).await;

    assert_eq!(answer.status, 201);
    let receipt = answer.json();
    let mut keys: Vec<&str> = receipt
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["receivedAt", "reference"]);
}

#[tokio::test]
async fn the_reference_is_the_stored_id_s_last_8_characters() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Which one?" });

    let receipt: Value = post_report(&browser, &served.base_url, &body)
        .await
        .json()
        .await
        .expect("the receipt is JSON");

    let report = only(stored_in_memory(&served).await);
    let id = report.id.to_string();
    let expected_reference = &id[id.len() - 8..];
    assert_eq!(receipt["reference"], expected_reference);
}

#[tokio::test]
async fn a_cross_origin_post_is_refused_and_stores_nothing() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "From elsewhere." });

    let refused = browser
        .post(format!("{}/bug-reports", served.base_url))
        .header("origin", "https://elsewhere.example")
        .json(&body)
        .send()
        .await
        .expect("POST /bug-reports");

    common::assert_problem(refused, 403, "cross_origin").await;
    assert!(stored_in_memory(&served).await.is_empty());
}

#[tokio::test]
async fn the_answer_is_no_store() {
    let (served, browser) = signed_in(reporter_runtime()).await;
    let body = json!({ "description": "Don't cache me." });

    let filed = post_report(&browser, &served.base_url, &body).await;

    assert_eq!(filed.status(), 201);
    let cache_control = filed
        .headers()
        .get("cache-control")
        .expect("a cache-control header");
    assert_eq!(cache_control, "no-store");
}

#[tokio::test]
async fn without_a_session_the_answer_is_401() {
    let served = serve(reporter_runtime(), api::app).await;
    let body = json!({ "description": "Who am I?" });

    let refused = post_report(&client(), &served.base_url, &body).await;

    common::assert_problem(refused, 401, "not_authenticated").await;
    assert!(stored_in_memory(&served).await.is_empty());
}
