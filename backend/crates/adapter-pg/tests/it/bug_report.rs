//! Bug reports over PostgreSQL, against a throwaway container: the table's
//! checks refuse every out-of-rule row by name, the reporter must be a User,
//! the locked count serializes two units and reads what was filed, the list
//! reads it too, and a full report and an empty one round-trip. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgBugReportStore, PgDatabase, PgPool};
use std::time::Duration;

use chrono::TimeDelta;
use domain::datetime::DateTimeUtc;
use domain::elements::actor_identity::ActorKind;
use domain::elements::bug_report::{
    BrowserText, BugDescription, BugReportId, BuildId, DisplaySettings, ErrorStatus, NewBugReport,
    PageAddress, PageKind, StoredBugReport, WindowSide, WindowSize,
};
use domain::elements::did::Did;
use domain::elements::text::StoredText;
use domain::elements::user::UserId;
use domain::ports::{BugReportStore, Database};

/// A fresh, fully migrated private database; the second element keeps the
/// shared container alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

fn at(instant: &str) -> DateTimeUtc {
    instant.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

/// Recognize `did` as a User, committed.
async fn provision(pool: &PgPool, did: &str) -> UserId {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user.id
}

/// A report with only its required items.
fn bare_report(reporter: &UserId, received_at: DateTimeUtc) -> NewBugReport {
    NewBugReport {
        id: BugReportId::from(uuid::Uuid::now_v7()),
        reporter: reporter.clone(),
        description: BugDescription::try_from("The page went blank").expect("a description"),
        page: None,
        page_kind: None,
        web_version: None,
        browser: None,
        window: None,
        display: None,
        error_status: None,
        api_version: BuildId::try_from("api-9.9.9").expect("a build id"),
        received_at,
    }
}

/// A report with every item set, each text distinct from every other.
fn full_report(
    reporter: &UserId,
    received_at: DateTimeUtc,
    display: DisplaySettings,
) -> NewBugReport {
    let window = WindowSize {
        width: WindowSide::try_from(1280).expect("a side"),
        height: WindowSide::try_from(720).expect("a side"),
    };
    NewBugReport {
        page: Some(PageAddress::try_from("/commissions/42").expect("a page")),
        page_kind: Some(PageKind::try_from("commission").expect("a page kind")),
        web_version: Some(BuildId::try_from("web-1.2.3").expect("a build id")),
        browser: Some(BrowserText::try_from("Firefox/131.0").expect("a browser")),
        window: Some(window),
        display: Some(display),
        error_status: Some(ErrorStatus::try_from(503).expect("a status")),
        ..bare_report(reporter, received_at)
    }
}

/// `report` as the store should read it back.
fn stored(report: &NewBugReport) -> StoredBugReport {
    let stored_text = |text: &str| StoredText::from(text.to_owned());
    StoredBugReport {
        id: report.id,
        reporter: report.reporter.clone(),
        description: stored_text(report.description.as_ref()),
        page: report.page.as_ref().map(|page| stored_text(page.as_ref())),
        page_kind: report
            .page_kind
            .as_ref()
            .map(|kind| stored_text(kind.as_ref())),
        web_version: report
            .web_version
            .as_ref()
            .map(|version| stored_text(version.as_ref())),
        browser: report
            .browser
            .as_ref()
            .map(|browser| stored_text(browser.as_ref())),
        window: report.window,
        display: report.display,
        error_status: report.error_status,
        api_version: stored_text(report.api_version.as_ref()),
        received_at: report.received_at,
    }
}

/// File `reports` through the port in one unit, committed.
async fn file(pool: &PgPool, reports: &[NewBugReport]) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    for report in reports {
        uow.bug_reports().file(report).await.expect("filed");
    }
    uow.commit().await.expect("commit");
}

// ---- The table's checks, by raw insert ----------------------------------

/// One `bug_report` row as raw column values; [`RawRow::valid`] passes every
/// check, and each test breaks one column.
#[derive(Clone)]
struct RawRow {
    reporter_id: String,
    description: String,
    page: Option<String>,
    page_kind: Option<String>,
    web_version: Option<String>,
    browser: Option<String>,
    window_width: Option<i32>,
    window_height: Option<i32>,
    reduced_motion: Option<bool>,
    more_contrast: Option<bool>,
    forced_colors: Option<bool>,
    error_status: Option<i16>,
    api_version: String,
}

impl RawRow {
    /// A row with every column set and within its rule.
    fn valid(reporter: &UserId) -> Self {
        Self {
            reporter_id: reporter.as_ref().to_owned(),
            description: "It broke".to_owned(),
            page: Some("/den".to_owned()),
            page_kind: Some("den".to_owned()),
            web_version: Some("web-1".to_owned()),
            browser: Some("Firefox".to_owned()),
            window_width: Some(800),
            window_height: Some(600),
            reduced_motion: Some(false),
            more_contrast: Some(false),
            forced_colors: Some(false),
            error_status: Some(404),
            api_version: "api-1".to_owned(),
        }
    }

    async fn insert(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO bug_report (
                 id, reporter_id, description, page, page_kind, web_version, browser,
                 window_width, window_height, reduced_motion, more_contrast, forced_colors,
                 error_status, api_version, received_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, now())",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(&self.reporter_id)
        .bind(&self.description)
        .bind(&self.page)
        .bind(&self.page_kind)
        .bind(&self.web_version)
        .bind(&self.browser)
        .bind(self.window_width)
        .bind(self.window_height)
        .bind(self.reduced_motion)
        .bind(self.more_contrast)
        .bind(self.forced_colors)
        .bind(self.error_status)
        .bind(&self.api_version)
        .execute(pool)
        .await
        .map(|_| ())
    }
}

/// Assert `row` is refused by the constraint named `constraint`, so a test
/// goes red when that constraint is dropped even if another one would also
/// refuse the row.
async fn assert_refused_by(pool: &PgPool, row: &RawRow, constraint: &str) {
    let refusal = row.insert(pool).await.expect_err("the row must be refused");
    let refused_by = refusal
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .map(str::to_owned);
    assert_eq!(
        refused_by.as_deref(),
        Some(constraint),
        "expected {constraint} to refuse the row, got: {refusal:?}"
    );
}

/// Assert `row` is stored.
async fn assert_accepted(pool: &PgPool, row: &RawRow) {
    row.insert(pool).await.expect("the row must be accepted");
}

#[tokio::test]
async fn the_valid_row_and_the_all_null_row_are_accepted() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-valid").await;
    let valid = RawRow::valid(&reporter);
    let all_null = RawRow {
        page: None,
        page_kind: None,
        web_version: None,
        browser: None,
        window_width: None,
        window_height: None,
        reduced_motion: None,
        more_contrast: None,
        forced_colors: None,
        error_status: None,
        ..valid.clone()
    };

    assert_accepted(&pool, &valid).await;
    assert_accepted(&pool, &all_null).await;
}

#[tokio::test]
async fn a_half_set_window_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-window").await;
    let valid = RawRow::valid(&reporter);
    let width_only = RawRow {
        window_height: None,
        ..valid.clone()
    };
    let height_only = RawRow {
        window_width: None,
        ..valid.clone()
    };

    assert_refused_by(&pool, &width_only, "bug_report_window_both_or_neither").await;
    assert_refused_by(&pool, &height_only, "bug_report_window_both_or_neither").await;
}

#[tokio::test]
async fn a_window_side_outside_1_to_100000_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-window-range").await;
    let valid = RawRow::valid(&reporter);
    let zero_width = RawRow {
        window_width: Some(0),
        ..valid.clone()
    };
    let wide_height = RawRow {
        window_height: Some(100_001),
        ..valid.clone()
    };
    let wide_width = RawRow {
        window_width: Some(100_001),
        ..valid.clone()
    };
    let zero_height = RawRow {
        window_height: Some(0),
        ..valid.clone()
    };
    let widest = RawRow {
        window_width: Some(100_000),
        window_height: Some(1),
        ..valid.clone()
    };

    assert_refused_by(&pool, &zero_width, "bug_report_window_width_range").await;
    assert_refused_by(&pool, &wide_height, "bug_report_window_height_range").await;
    assert_refused_by(&pool, &wide_width, "bug_report_window_width_range").await;
    assert_refused_by(&pool, &zero_height, "bug_report_window_height_range").await;
    assert_accepted(&pool, &widest).await;
}

#[tokio::test]
async fn one_or_two_display_settings_are_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-display").await;
    let valid = RawRow::valid(&reporter);
    let one_set = RawRow {
        more_contrast: None,
        forced_colors: None,
        ..valid.clone()
    };
    let two_set = RawRow {
        reduced_motion: None,
        ..valid.clone()
    };

    assert_refused_by(&pool, &one_set, "bug_report_display_all_or_none").await;
    assert_refused_by(&pool, &two_set, "bug_report_display_all_or_none").await;
}

#[tokio::test]
async fn a_page_with_a_query_or_fragment_or_no_single_leading_slash_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-page").await;
    let valid = RawRow::valid(&reporter);
    let with_fragment = RawRow {
        page: Some("/den#files".to_owned()),
        ..valid.clone()
    };
    let with_query = RawRow {
        page: Some("/den?tab=files".to_owned()),
        ..valid.clone()
    };
    let relative = RawRow {
        page: Some("den/files".to_owned()),
        ..valid.clone()
    };
    let protocol_relative = RawRow {
        page: Some("//evil.example/x".to_owned()),
        ..valid.clone()
    };
    let double_slash_inside = RawRow {
        page: Some("/den//x".to_owned()),
        ..valid.clone()
    };

    assert_refused_by(
        &pool,
        &with_fragment,
        "bug_report_page_no_query_or_fragment",
    )
    .await;
    assert_refused_by(&pool, &with_query, "bug_report_page_no_query_or_fragment").await;
    assert_refused_by(&pool, &relative, "bug_report_page_starts_with_slash").await;
    assert_refused_by(
        &pool,
        &protocol_relative,
        "bug_report_page_not_protocol_relative",
    )
    .await;
    assert_accepted(&pool, &double_slash_inside).await;
}

#[tokio::test]
async fn a_description_is_1_to_4000_code_points() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-description").await;
    let valid = RawRow::valid(&reporter);
    let empty = RawRow {
        description: String::new(),
        ..valid.clone()
    };
    let over_cap = RawRow {
        description: "a".repeat(4001),
        ..valid.clone()
    };
    let four_thousand_emoji = RawRow {
        description: "🦊".repeat(4000),
        ..valid.clone()
    };

    assert_refused_by(&pool, &empty, "bug_report_description_length").await;
    assert_refused_by(&pool, &over_cap, "bug_report_description_length").await;
    assert_accepted(&pool, &four_thousand_emoji).await;
}

#[tokio::test]
async fn each_optional_text_holds_its_byte_cap_and_no_more() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-caps").await;
    let valid = RawRow::valid(&reporter);
    let page_of = |bytes: usize| format!("/{}", "a".repeat(bytes - 1));
    let page_at_cap = RawRow {
        page: Some(page_of(2048)),
        ..valid.clone()
    };
    let page_over_cap = RawRow {
        page: Some(page_of(2049)),
        ..valid.clone()
    };
    let page_kind_at_cap = RawRow {
        page_kind: Some("k".repeat(64)),
        ..valid.clone()
    };
    let page_kind_over_cap = RawRow {
        page_kind: Some("k".repeat(65)),
        ..valid.clone()
    };
    let web_version_at_cap = RawRow {
        web_version: Some("w".repeat(64)),
        ..valid.clone()
    };
    let web_version_over_cap = RawRow {
        web_version: Some("w".repeat(65)),
        ..valid.clone()
    };
    let browser_at_cap = RawRow {
        browser: Some("b".repeat(512)),
        ..valid.clone()
    };
    let browser_over_cap = RawRow {
        browser: Some("b".repeat(513)),
        ..valid.clone()
    };

    assert_accepted(&pool, &page_at_cap).await;
    assert_refused_by(&pool, &page_over_cap, "bug_report_page_max_bytes").await;
    assert_accepted(&pool, &page_kind_at_cap).await;
    assert_refused_by(&pool, &page_kind_over_cap, "bug_report_page_kind_max_bytes").await;
    assert_accepted(&pool, &web_version_at_cap).await;
    assert_refused_by(
        &pool,
        &web_version_over_cap,
        "bug_report_web_version_max_bytes",
    )
    .await;
    assert_accepted(&pool, &browser_at_cap).await;
    assert_refused_by(&pool, &browser_over_cap, "bug_report_browser_max_bytes").await;
}

#[tokio::test]
async fn a_byte_cap_counts_bytes_not_characters() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-bytes").await;
    let valid = RawRow::valid(&reporter);
    // Each over its cap in bytes and under it in characters: "é" is two bytes.
    let page_over_cap_in_bytes = RawRow {
        page: Some(format!("/{}", "é".repeat(1024))),
        ..valid.clone()
    };
    let page_kind_over_cap_in_bytes = RawRow {
        page_kind: Some("é".repeat(33)),
        ..valid.clone()
    };
    let web_version_over_cap_in_bytes = RawRow {
        web_version: Some("é".repeat(33)),
        ..valid.clone()
    };
    let browser_over_cap_in_bytes = RawRow {
        browser: Some("é".repeat(257)),
        ..valid.clone()
    };

    assert_refused_by(&pool, &page_over_cap_in_bytes, "bug_report_page_max_bytes").await;
    assert_refused_by(
        &pool,
        &page_kind_over_cap_in_bytes,
        "bug_report_page_kind_max_bytes",
    )
    .await;
    assert_refused_by(
        &pool,
        &web_version_over_cap_in_bytes,
        "bug_report_web_version_max_bytes",
    )
    .await;
    assert_refused_by(
        &pool,
        &browser_over_cap_in_bytes,
        "bug_report_browser_max_bytes",
    )
    .await;
}

#[tokio::test]
async fn an_api_version_is_1_to_64_bytes() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-api-version").await;
    let valid = RawRow::valid(&reporter);
    let empty = RawRow {
        api_version: String::new(),
        ..valid.clone()
    };
    // 33 two-byte characters: 66 bytes, under 65 characters.
    let over_cap_in_bytes = RawRow {
        api_version: "é".repeat(33),
        ..valid.clone()
    };
    let at_cap = RawRow {
        api_version: "v".repeat(64),
        ..valid.clone()
    };

    assert_refused_by(&pool, &empty, "bug_report_api_version_bytes").await;
    assert_refused_by(&pool, &over_cap_in_bytes, "bug_report_api_version_bytes").await;
    assert_accepted(&pool, &at_cap).await;
}

#[tokio::test]
async fn an_error_status_outside_400_to_599_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-status").await;
    let valid = RawRow::valid(&reporter);
    let below = RawRow {
        error_status: Some(399),
        ..valid.clone()
    };
    let above = RawRow {
        error_status: Some(600),
        ..valid.clone()
    };
    let lowest = RawRow {
        error_status: Some(400),
        ..valid.clone()
    };
    let highest = RawRow {
        error_status: Some(599),
        ..valid.clone()
    };

    assert_refused_by(&pool, &below, "bug_report_error_status_range").await;
    assert_refused_by(&pool, &above, "bug_report_error_status_range").await;
    assert_accepted(&pool, &lowest).await;
    assert_accepted(&pool, &highest).await;
}

#[tokio::test]
async fn an_empty_optional_text_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:raw-empty").await;
    let valid = RawRow::valid(&reporter);
    let empty_page = RawRow {
        page: Some(String::new()),
        ..valid.clone()
    };
    let empty_page_kind = RawRow {
        page_kind: Some(String::new()),
        ..valid.clone()
    };
    let empty_web_version = RawRow {
        web_version: Some(String::new()),
        ..valid.clone()
    };
    let empty_browser = RawRow {
        browser: Some(String::new()),
        ..valid.clone()
    };

    // An empty page also fails the leading-slash check, which Postgres runs
    // after this one: checks run in name order.
    assert_refused_by(&pool, &empty_page, "bug_report_page_not_empty").await;
    assert_refused_by(&pool, &empty_page_kind, "bug_report_page_kind_not_empty").await;
    assert_refused_by(
        &pool,
        &empty_web_version,
        "bug_report_web_version_not_empty",
    )
    .await;
    assert_refused_by(&pool, &empty_browser, "bug_report_browser_not_empty").await;
}

// ---- The reporter --------------------------------------------------------

#[tokio::test]
async fn a_report_whose_reporter_is_an_account_fails_the_foreign_key() {
    let (pool, _container) = fresh_pool().await;
    let account_did = Did::from("did:plc:an-account".to_owned());
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.actor_identities()
        .intern(&account_did, ActorKind::Account, at("2026-10-17T12:00:00Z"))
        .await
        .expect("intern the Account's DID");
    uow.commit().await.expect("commit");
    let report = bare_report(&UserId::from(account_did), at("2026-10-17T12:00:00Z"));

    let mut uow = db.begin().await.expect("begin");
    let refusal = uow
        .bug_reports()
        .file(&report)
        .await
        .expect_err("an Account is no User");

    let database_error = refusal
        .downcast_ref::<sqlx::Error>()
        .and_then(|error| error.as_database_error())
        .expect("a database error");
    assert_eq!(database_error.code().as_deref(), Some("23503"));
    assert_eq!(database_error.constraint(), Some("bug_report_reporter"));
}

// ---- The locked count and the list -----------------------------------------

/// Lock `reporter` and count their reports since `since` in a fresh unit, then
/// roll it back.
async fn locked_count(pool: &PgPool, reporter: &UserId, since: DateTimeUtc) -> i64 {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let filed = uow
        .bug_reports()
        .lock_reporter_and_count_since(reporter, since)
        .await
        .expect("a count");
    uow.rollback().await.expect("rollback");
    filed
}

#[tokio::test]
async fn lock_reporter_and_count_since_counts_only_this_reporter() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:count-reporter").await;
    let other = provision(&pool, "did:plc:count-other").await;
    let since = at("2026-10-17T12:00:00Z");
    let reports = [
        bare_report(&reporter, since),
        bare_report(&other, since),
        bare_report(&other, since),
    ];
    file(&pool, &reports).await;

    let filed = locked_count(&pool, &reporter, since).await;

    assert_eq!(filed, 1);
}

#[tokio::test]
async fn lock_reporter_and_count_since_window_boundary() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:count-boundary").await;
    let since = at("2026-10-17T12:00:00Z");
    let just_before = since - TimeDelta::microseconds(1);
    let reports = [
        bare_report(&reporter, since),
        bare_report(&reporter, just_before),
    ];
    file(&pool, &reports).await;

    let filed = locked_count(&pool, &reporter, since).await;

    assert_eq!(filed, 1);
}

#[tokio::test]
async fn a_second_unit_waits_for_the_lock_then_counts_the_first_unit_s_report() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:count-serialized").await;
    let since = at("2026-10-17T12:00:00Z");
    let db = PgDatabase::new(pool.clone());
    let mut first = db.begin().await.expect("begin");
    let first_count = first
        .bug_reports()
        .lock_reporter_and_count_since(&reporter, since)
        .await
        .expect("a count");
    first
        .bug_reports()
        .file(&bare_report(&reporter, since))
        .await
        .expect("filed");

    let second_db = PgDatabase::new(pool.clone());
    let second_reporter = reporter.clone();
    let mut second = tokio::spawn(async move {
        let mut uow = second_db.begin().await.expect("begin");
        let filed = uow
            .bug_reports()
            .lock_reporter_and_count_since(&second_reporter, since)
            .await
            .expect("a count");
        uow.rollback().await.expect("rollback");
        filed
    });
    let counted_while_the_first_held =
        tokio::time::timeout(Duration::from_millis(500), &mut second)
            .await
            .is_ok();
    assert!(
        !counted_while_the_first_held,
        "the second unit counted while the first held the lock"
    );
    first.commit().await.expect("commit");
    let second_count = second.await.expect("the second unit ran");

    assert_eq!(first_count, 0);
    assert_eq!(second_count, 1, "the second unit counts the first's report");
}

#[tokio::test]
async fn list_since_filters_and_orders() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:list").await;
    let now = at("2026-10-17T12:00:00Z");
    let before = bare_report(&reporter, now - TimeDelta::microseconds(1));
    let later = bare_report(&reporter, now + TimeDelta::seconds(1));
    let mut tie_first = bare_report(&reporter, now);
    tie_first.id = BugReportId::from(uuid::Uuid::from_u128(1));
    let mut tie_second = bare_report(&reporter, now);
    tie_second.id = BugReportId::from(uuid::Uuid::from_u128(2));
    let reports = [
        later.clone(),
        tie_second.clone(),
        before.clone(),
        tie_first.clone(),
    ];
    file(&pool, &reports).await;
    let store = PgBugReportStore::new(pool.clone());

    let since_now = store.list_since(Some(now)).await.expect("a list");
    let everything = store.list_since(None).await.expect("a list");

    let since_now_ids: Vec<BugReportId> = since_now.iter().map(|report| report.id).collect();
    let expected_since_now = vec![tie_first.id, tie_second.id, later.id];
    assert_eq!(since_now_ids, expected_since_now);
    let everything_ids: Vec<BugReportId> = everything.iter().map(|report| report.id).collect();
    let expected_everything = vec![before.id, tie_first.id, tie_second.id, later.id];
    assert_eq!(everything_ids, expected_everything);
}

// ---- Round trips -----------------------------------------------------------

#[tokio::test]
async fn a_full_report_round_trips() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:full").await;
    // Two display variants, so a swap of any two settings changes one of them.
    let first_display = DisplaySettings {
        reduced_motion: true,
        more_contrast: false,
        forced_colors: false,
    };
    let second_display = DisplaySettings {
        reduced_motion: false,
        more_contrast: true,
        forced_colors: false,
    };
    let first = full_report(&reporter, at("2026-10-17T12:00:00.123456Z"), first_display);
    let second = full_report(&reporter, at("2026-10-17T12:00:01.654321Z"), second_display);
    file(&pool, &[first.clone(), second.clone()]).await;

    let listed = PgBugReportStore::new(pool.clone())
        .list_since(None)
        .await
        .expect("a list");

    let expected = vec![stored(&first), stored(&second)];
    assert_eq!(listed, expected);
}

#[tokio::test]
async fn an_empty_report_stores_nulls() {
    let (pool, _container) = fresh_pool().await;
    let reporter = provision(&pool, "did:plc:empty").await;
    let report = bare_report(&reporter, at("2026-10-17T12:00:00Z"));
    file(&pool, std::slice::from_ref(&report)).await;

    let null_columns: i32 = sqlx::query_scalar(
        "SELECT num_nulls(page, page_kind, web_version, browser, window_width, window_height,
                          reduced_motion, more_contrast, forced_colors, error_status)
         FROM bug_report WHERE id = $1",
    )
    .bind(uuid::Uuid::from(report.id))
    .fetch_one(&pool)
    .await
    .expect("the row");
    let listed = PgBugReportStore::new(pool.clone())
        .list_since(None)
        .await
        .expect("a list");

    assert_eq!(null_columns, 10);
    let expected = vec![stored(&report)];
    assert_eq!(listed, expected);
    assert_eq!(listed[0].window, None);
    assert_eq!(listed[0].display, None);
}
