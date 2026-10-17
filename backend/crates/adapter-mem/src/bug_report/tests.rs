use chrono::TimeDelta;
use domain::elements::bug_report::{BugDescription, BugReportId, BuildId};

use super::*;

fn at(instant: &str) -> DateTimeUtc {
    instant.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

fn did(text: &str) -> Did {
    Did::from(text.to_owned())
}

/// A report with only its required items, filed by `reporter` at `received_at`.
fn report(reporter: &Did, received_at: DateTimeUtc) -> NewBugReport {
    NewBugReport {
        id: BugReportId::from(uuid::Uuid::now_v7()),
        reporter: UserId::from(reporter.clone()),
        description: BugDescription::try_from("The page went blank").expect("a description"),
        page: None,
        page_kind: None,
        web_version: None,
        browser: None,
        window: None,
        display: None,
        error_status: None,
        api_version: BuildId::try_from("api-test-build").expect("a build id"),
        received_at,
    }
}

/// File `reports` in one unit and commit it.
async fn file_committed(backend: &MemBackend, reports: &[NewBugReport]) {
    let mut uow = backend.database().begin().await.expect("a unit");
    for new_report in reports {
        uow.bug_reports().file(new_report).await.expect("filed");
    }
    uow.commit().await.expect("committed");
}

#[tokio::test]
async fn filing_refuses_a_reporter_who_is_not_a_user() {
    let backend = MemBackend::new();
    let stranger = did("did:plc:stranger");
    let new_report = report(&stranger, at("2026-10-17T12:00:00Z"));

    let mut uow = backend.database().begin().await.expect("a unit");
    let filed = uow.bug_reports().file(&new_report).await;

    assert!(filed.is_err());
}

#[tokio::test]
async fn count_filed_since_window_boundary() {
    let backend = MemBackend::new();
    let reporter = did("did:plc:reporter");
    backend.provision(&reporter).await.expect("a User");
    let since = at("2026-10-17T12:00:00Z");
    let just_before = since - TimeDelta::microseconds(1);
    let reports = [report(&reporter, since), report(&reporter, just_before)];
    file_committed(&backend, &reports).await;

    let filed = backend
        .bug_report_store()
        .count_filed_since(&UserId::from(reporter), since)
        .await
        .expect("a count");

    assert_eq!(filed, 1);
}

#[tokio::test]
async fn count_filed_since_counts_only_this_reporter() {
    let backend = MemBackend::new();
    let reporter = did("did:plc:reporter");
    let other = did("did:plc:other");
    backend.provision(&reporter).await.expect("a User");
    backend.provision(&other).await.expect("a User");
    let since = at("2026-10-17T12:00:00Z");
    let reports = [report(&reporter, since), report(&other, since)];
    file_committed(&backend, &reports).await;

    let filed = backend
        .bug_report_store()
        .count_filed_since(&UserId::from(reporter), since)
        .await
        .expect("a count");

    assert_eq!(filed, 1);
}

#[tokio::test]
async fn a_rolled_back_unit_leaves_no_report() {
    let backend = MemBackend::new();
    let reporter = did("did:plc:reporter");
    backend.provision(&reporter).await.expect("a User");
    let new_report = report(&reporter, at("2026-10-17T12:00:00Z"));

    let mut rolled_back = backend.database().begin().await.expect("a unit");
    rolled_back
        .bug_reports()
        .file(&new_report)
        .await
        .expect("filed");
    rolled_back.rollback().await.expect("rolled back");
    let mut dropped = backend.database().begin().await.expect("a unit");
    dropped
        .bug_reports()
        .file(&new_report)
        .await
        .expect("filed");
    drop(dropped);

    let listed = backend
        .bug_report_store()
        .list_since(None)
        .await
        .expect("a list");
    assert!(listed.is_empty());
}

#[tokio::test]
async fn a_committed_report_reads_back_whole() {
    let backend = MemBackend::new();
    let reporter = did("did:plc:reporter");
    backend.provision(&reporter).await.expect("a User");
    let new_report = report(&reporter, at("2026-10-17T12:00:00Z"));
    file_committed(&backend, std::slice::from_ref(&new_report)).await;

    let listed = backend
        .bug_report_store()
        .list_since(None)
        .await
        .expect("a list");

    let expected = vec![stored_report(&new_report)];
    assert_eq!(listed, expected);
    assert_eq!(listed[0].description.as_str(), "The page went blank");
}

#[tokio::test]
async fn list_since_filters_and_orders() {
    let backend = MemBackend::new();
    let reporter = did("did:plc:reporter");
    backend.provision(&reporter).await.expect("a User");
    let since = at("2026-10-17T12:00:00Z");
    let later = since + TimeDelta::seconds(1);
    let before = report(&reporter, since - TimeDelta::microseconds(1));
    let late = report(&reporter, later);
    let mut tie_first = report(&reporter, since);
    tie_first.id = BugReportId::from(uuid::Uuid::from_u128(1));
    let mut tie_second = report(&reporter, since);
    tie_second.id = BugReportId::from(uuid::Uuid::from_u128(2));
    let reports = [late.clone(), tie_second.clone(), before, tie_first.clone()];
    file_committed(&backend, &reports).await;

    let listed = backend
        .bug_report_store()
        .list_since(Some(since))
        .await
        .expect("a list");

    let listed_ids: Vec<BugReportId> = listed.iter().map(|stored| stored.id).collect();
    let expected_ids = vec![tie_first.id, tie_second.id, late.id];
    assert_eq!(listed_ids, expected_ids);
}
