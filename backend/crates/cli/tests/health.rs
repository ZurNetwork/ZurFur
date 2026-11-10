//! `zurfur health` end to end: a reachable database answers
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

// `health` holds no purpose, so it never reads the custody root: a malformed one is
// no `config` problem, and its value never reaches stderr.
#[tokio::test]
async fn health_never_reads_the_root() {
    const MALFORMED_ROOT: &str = "987654321012345";
    let db = test_support::pg::fresh_db().await;
    let url = db.url().to_string();

    let output = tokio::task::spawn_blocking(move || {
        zurfur(&url)
            .env(composition::ROOT_KEY_ENV, MALFORMED_ROOT)
            .args(["--json", "health"])
            .output()
            .expect("zurfur runs")
    })
    .await
    .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(MALFORMED_ROOT), "{stderr}");
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "ok");
}
