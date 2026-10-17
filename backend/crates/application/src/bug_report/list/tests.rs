use chrono::{Duration, TimeZone, Utc};
use domain::elements::{
    bug_report::{BugDescription, BuildId},
    did::Did,
    user::UserId,
};

use super::*;
use crate::bug_report::file::{Command, ReportBody};

/// The instant the earliest report is filed at.
fn t0() -> DateTimeUtc {
    Utc.with_ymd_and_hms(2026, 10, 17, 9, 0, 0)
        .single()
        .expect("a valid instant")
}

/// The in-memory ports with one provisioned reporter, and that reporter.
async fn ports_with_reporter() -> (Ports, UserId) {
    let reporter = UserId::from(Did::from("did:plc:bugreportlister".to_owned()));
    let built = test_support::runtime::mem(reporter.did()).build();
    built
        .backend
        .provision(reporter.did())
        .await
        .expect("the reporter is provisioned");
    let ports = Ports::from(&built.runtime);
    (ports, reporter)
}

/// Files a report described as `description`, received at `now`.
async fn file_at(ports: &Ports, reporter: &UserId, description: &str, now: DateTimeUtc) {
    let report = ReportBody {
        description: BugDescription::try_from(description).expect("a description"),
        page: None,
        page_kind: None,
        web_version: None,
        browser: None,
        window: None,
        display: None,
        error_status: None,
    };
    let command = Command {
        reporter: reporter.clone(),
        report,
    };
    let api_version = BuildId::try_from("bugtest-list").expect("a build id");
    BugReports::from(ports)
        .file(command, &api_version, now)
        .await
        .expect("the report is filed");
}

/// The descriptions of `output`'s reports, in order.
fn descriptions(output: &Output) -> Vec<&str> {
    output
        .reports
        .iter()
        .map(|report| report.description.as_ref())
        .collect()
}

#[tokio::test]
async fn reports_list_oldest_first() {
    let (ports, reporter) = ports_with_reporter().await;
    file_at(&ports, &reporter, "third", t0() + Duration::hours(2)).await;
    file_at(&ports, &reporter, "first", t0()).await;
    file_at(&ports, &reporter, "second", t0() + Duration::hours(1)).await;

    let listed = BugReports::from(&ports)
        .list(Query::default())
        .await
        .expect("the reports list");

    assert_eq!(descriptions(&listed), ["first", "second", "third"]);
}

#[tokio::test]
async fn since_leaves_out_older_reports() {
    let (ports, reporter) = ports_with_reporter().await;
    file_at(&ports, &reporter, "first", t0()).await;
    file_at(&ports, &reporter, "second", t0() + Duration::hours(1)).await;
    file_at(&ports, &reporter, "third", t0() + Duration::hours(2)).await;

    let since_the_second = Query {
        since: Some(t0() + Duration::hours(1)),
    };
    let listed = BugReports::from(&ports)
        .list(since_the_second)
        .await
        .expect("the reports list");

    assert_eq!(descriptions(&listed), ["second", "third"]);
}
