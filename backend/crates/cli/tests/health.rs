//! `zurfur health` end to end (ZMVP-202): a reachable database answers
//! `ok` with exit 0; an unreachable one is an infrastructure problem with a
//! distinct code and exit 3. The reachable case rides the shared
//! testcontainers Postgres, like the rest of the workspace.

mod common;
use common::{problem, zurfur};

#[tokio::test]
async fn a_reachable_database_reports_ok() {
    let db = test_support::pg::fresh_db().await;
    let url = db.url().to_string();
    let output = tokio::task::spawn_blocking(move || {
        zurfur(&url).args(["health", "--json"]).assert().success()
    })
    .await
    .unwrap();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let report: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(report["status"], "ok");
    assert_eq!(report["database"], "up");
    assert!(report["latency_ms"].is_number());
    assert!(
        output.get_output().stderr.is_empty(),
        "with RUST_LOG=off nothing may reach stderr on success"
    );
}

// The runtime cannot even connect: `service_unavailable`, detail = the
// connect failure (the probe's own timeout is covered in `tests/in_process.rs`).
#[test]
fn an_unreachable_database_is_a_distinct_infra_problem() {
    // Port 1 on loopback: refused immediately, nobody listens there.
    let output = zurfur("postgres://nobody:nothing@127.0.0.1:1/nothing")
        .arg("health")
        .assert()
        .code(3);
    assert!(output.get_output().stdout.is_empty());
    let problem = problem(&output.get_output().stderr);
    assert_eq!(problem["class"], "infra");
    assert_eq!(problem["code"], "service_unavailable");
    assert!(
        problem["detail"]
            .as_str()
            .unwrap()
            .contains("database unreachable")
    );
}
