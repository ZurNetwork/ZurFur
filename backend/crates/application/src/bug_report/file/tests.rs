use std::sync::Arc;

use chrono::{Duration, TimeZone, Utc};
use domain::elements::did::Did;
use test_support::recording::{CallLog, RecordingDatabase};

use super::*;
use crate::Ports;

/// The instant the first reports are filed at.
fn t0() -> DateTimeUtc {
    Utc.with_ymd_and_hms(2026, 10, 17, 12, 0, 0)
        .single()
        .expect("a valid instant")
}

fn alice() -> UserId {
    UserId::from(Did::from("did:plc:bugreportalice".to_owned()))
}

fn bob() -> UserId {
    UserId::from(Did::from("did:plc:bugreportbob".to_owned()))
}

fn api_version() -> BuildId {
    BuildId::try_from("bugtest-unit").expect("a build id")
}

/// A report holding only its description.
fn command_for(reporter: UserId) -> Command {
    let report = ReportBody {
        description: BugDescription::try_from("The board froze.").expect("a description"),
        page: None,
        page_kind: None,
        web_version: None,
        browser: None,
        window: None,
        display: None,
        error_status: None,
    };
    Command { reporter, report }
}

/// The in-memory ports with alice and bob provisioned, the database logging
/// into the returned [`CallLog`].
async fn recording_ports() -> (Ports, CallLog) {
    let built = test_support::runtime::mem(alice().did()).build();
    built
        .backend
        .provision(alice().did())
        .await
        .expect("alice is provisioned");
    built
        .backend
        .provision(bob().did())
        .await
        .expect("bob is provisioned");
    let log = CallLog::default();
    let database = RecordingDatabase::new(built.runtime.database.clone(), log.clone());
    let ports = Ports {
        database: Arc::new(database),
        ..Ports::from(&built.runtime)
    };
    (ports, log)
}

/// Files `count` reports as `reporter` at `now`, asserting each is accepted.
async fn file_accepted(ports: &Ports, reporter: &UserId, count: usize, now: DateTimeUtc) {
    for nth in 1..=count {
        let filed = BugReports::from(ports)
            .file(command_for(reporter.clone()), &api_version(), now)
            .await;
        assert!(filed.is_ok(), "report {nth} is accepted: {filed:?}");
    }
}

#[tokio::test]
async fn the_11th_is_refused_59_minutes_later() {
    let (ports, _log) = recording_ports().await;
    file_accepted(&ports, &alice(), 10, t0()).await;

    let fifty_nine_minutes_later = t0() + Duration::minutes(59);
    let eleventh = BugReports::from(&ports)
        .file(
            command_for(alice()),
            &api_version(),
            fifty_nine_minutes_later,
        )
        .await;
    assert!(
        matches!(eleventh, Err(BugReportError::RateLimited)),
        "the 11th within the hour is refused: {eleventh:?}"
    );

    let past_the_window = t0() + Duration::minutes(60) + Duration::seconds(1);
    let after_the_window = BugReports::from(&ports)
        .file(command_for(alice()), &api_version(), past_the_window)
        .await;
    assert!(
        after_the_window.is_ok(),
        "the ten at t0 have left the window: {after_the_window:?}"
    );
}

#[tokio::test]
async fn bob_s_reports_do_not_count_for_alice() {
    let (ports, _log) = recording_ports().await;
    file_accepted(&ports, &bob(), 10, t0()).await;

    let alice_files = BugReports::from(&ports)
        .file(command_for(alice()), &api_version(), t0())
        .await;
    assert!(
        alice_files.is_ok(),
        "bob's ten are his own: {alice_files:?}"
    );
}

#[tokio::test]
async fn a_refused_report_stores_nothing() {
    let (ports, log) = recording_ports().await;
    file_accepted(&ports, &alice(), 10, t0()).await;
    let events_before = log.events().len();

    let refused = BugReports::from(&ports)
        .file(command_for(alice()), &api_version(), t0())
        .await;

    assert!(
        matches!(refused, Err(BugReportError::RateLimited)),
        "the 11th is refused: {refused:?}"
    );
    let stored = ports
        .bug_reports
        .list_since(None)
        .await
        .expect("the reports list");
    assert_eq!(stored.len(), 10, "the refused report is not stored");
    let refusal_events = log.events()[events_before..].to_vec();
    assert_eq!(
        refusal_events,
        ["begin", "rollback"],
        "the refusal counted in a unit and rolled it back"
    );
}

#[tokio::test]
async fn the_receipt_names_the_stored_report() {
    let (ports, _log) = recording_ports().await;

    let receipt = BugReports::from(&ports)
        .file(command_for(alice()), &api_version(), t0())
        .await
        .expect("the report is filed");

    let stored = ports
        .bug_reports
        .list_since(None)
        .await
        .expect("the reports list");
    let [report] = stored.as_slice() else {
        panic!("one report is stored: {stored:?}");
    };
    let expected_reference = BugReportReference::from(&report.id);
    assert_eq!(receipt.reference, expected_reference);
    assert_eq!(receipt.received_at, t0());
    assert_eq!(report.received_at, t0());
    assert_eq!(report.reporter, alice());
    assert_eq!(report.api_version.as_ref(), "bugtest-unit");
}
