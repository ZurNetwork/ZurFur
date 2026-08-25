//! `zurfur migrate` + the schema-drift gate end to end (ZMVP-206), on a
//! **bare** throwaway Postgres: data commands refuse until `migrate` runs;
//! `health` reports instead of refusing; `migrate` is idempotent.

mod common;
use assert_cmd::Command;
use common::problem as json;

/// The shared harness plus the identity file redirected into `home`.
fn zurfur(database_url: &str, home: &std::path::Path) -> Command {
    let mut cmd = common::zurfur(database_url);
    cmd.env(cli::identity::HOME_ENV, home);
    cmd
}

#[tokio::test]
async fn a_bare_database_is_refused_until_migrated() {
    let db = test_support::pg::bare_db().await;
    let url = db.url().to_string();
    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().unwrap();

        // Data command on a bare DB: refused, and told the fix.
        let output = zurfur(&url, home.path())
            .args(["session", "whoami"])
            .assert()
            .code(3);
        let problem = json(&output.get_output().stderr);
        assert_eq!(problem["code"], "service_unavailable");
        assert!(
            problem["detail"]
                .as_str()
                .unwrap()
                .contains("zurfur migrate")
        );

        // Health reports, never refuses.
        let output = zurfur(&url, home.path())
            .args(["--json", "health"])
            .assert()
            .success();
        assert_eq!(json(&output.get_output().stdout)["schema"], "unknown");

        // Migrate applies everything; a second run applies nothing.
        let output = zurfur(&url, home.path())
            .args(["--json", "migrate"])
            .assert()
            .success();
        let report = json(&output.get_output().stdout);
        assert!(report["applied"].as_u64().unwrap() > 0, "{report}");
        assert!(report["version"].is_number());
        let output = zurfur(&url, home.path())
            .args(["--json", "migrate"])
            .assert()
            .success();
        assert_eq!(json(&output.get_output().stdout)["applied"], 0);

        // Now current: health says so, and the data command passes the gate
        // (failing only on the missing identity, which is the next problem).
        let output = zurfur(&url, home.path())
            .args(["--json", "health"])
            .assert()
            .success();
        assert_eq!(json(&output.get_output().stdout)["schema"], "current");
        let output = zurfur(&url, home.path())
            .args(["session", "whoami"])
            .assert()
            .code(1);
        assert_eq!(
            json(&output.get_output().stderr)["code"],
            "not_authenticated"
        );
    })
    .await
    .unwrap();
}
