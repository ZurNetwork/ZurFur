//! `zurfur session …` end to end (ZMVP-203) against a real throwaway
//! Postgres, with the identity file redirected into a temp dir through
//! `ZURFUR_CLI_HOME`. Pins the exit classes and the JSON on both channels.

mod common;
use assert_cmd::Command;
use common::problem;

/// The shared harness plus the identity file redirected into `home`.
fn zurfur(database_url: &str, home: &std::path::Path) -> Command {
    let mut cmd = common::zurfur(database_url);
    cmd.env(cli::identity::HOME_ENV, home);
    cmd
}

#[tokio::test]
async fn whoami_then_logout_twice() {
    let db = test_support::pg::fresh_db().await;
    let url = db.url().to_string();
    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().unwrap();

        // Nothing recorded yet: a domain problem, exit 1, stdout silent.
        let output = zurfur(&url, home.path())
            .args(["session", "whoami"])
            .assert()
            .code(1);
        assert!(output.get_output().stdout.is_empty());
        assert_eq!(
            problem(&output.get_output().stderr)["code"],
            "not_authenticated"
        );

        // Logout is idempotent: exit 0 both times, compact JSON under --json.
        for expected_had_identity in [false, false] {
            let output = zurfur(&url, home.path())
                .args(["--json", "session", "logout"])
                .assert()
                .success();
            let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
            assert_eq!(stdout.lines().count(), 1, "one compact line: {stdout:?}");
            let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
            assert_eq!(value["loggedOut"], true);
            assert_eq!(value["hadIdentity"], expected_had_identity);
        }

        // `login` is blocked on the ruling: honest infra problem, exit 3.
        let output = zurfur(&url, home.path())
            .args(["session", "login"])
            .assert()
            .code(3);
        assert_eq!(
            problem(&output.get_output().stderr)["code"],
            "not_implemented"
        );
    })
    .await
    .unwrap();
}
