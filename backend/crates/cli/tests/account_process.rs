//! `zurfur account create` / `delete` end to end against a real throwaway
//! Postgres: clap's subcommand + flag spellings, the stdout JSON, and the exit
//! classes — the process-level counterpart of `tests/account.rs`. Also the
//! operator's `bug-reports list` over a hostile row written past every rule.

mod common;
use adapter_pg::PgDatabase;
use assert_cmd::Command;
use common::problem;
use domain::elements::did::Did;
use domain::ports::Database;

const DID: &str = "did:plc:cli-account-process";
const DELETE_DID: &str = "did:plc:cli-account-delete-process";
const REPORTER_DID: &str = "did:plc:cli-bug-reports-process";

fn zurfur(database_url: &str, home: &std::path::Path) -> Command {
    let mut cmd = common::zurfur(database_url);
    cmd.env(cli::identity::HOME_ENV, home);
    cmd
}

#[tokio::test]
async fn account_create_over_the_binary() {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let url = db.url().to_string();

    // Recognize the visitor and record their identity — what `login` will do.
    let database = PgDatabase::new(pool);
    let mut uow = database.begin().await.expect("begin");
    uow.users()
        .provision(&Did::from(DID.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    let home = tempfile::tempdir().unwrap();
    let identity_path = home.path().join(cli::identity::IDENTITY_FILE_NAME);
    cli::identity::save(&identity_path, &cli::identity::Identity::new(DID, &url)).unwrap();

    tokio::task::spawn_blocking(move || {
        // Success: exit 0, one compact JSON line with the four projected keys.
        let output = zurfur(&url, home.path())
            .args([
                "--json",
                "account",
                "create",
                "--name",
                "Acme Studio",
                "--handle",
                "acme.zurfur.app",
            ])
            .assert()
            .success();
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        assert_eq!(stdout.lines().count(), 1, "one compact line: {stdout:?}");
        let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(value["handle"], "acme.zurfur.app");
        assert_eq!(value["name"], "Acme Studio");
        assert!(value["did"].as_str().unwrap().starts_with("did:plc:"));
        assert!(value["id"].as_str().is_some());

        // The same handle again: a domain problem, exit 1, stdout silent.
        let output = zurfur(&url, home.path())
            .args([
                "account",
                "create",
                "--name",
                "Twin",
                "--handle",
                "acme.zurfur.app",
            ])
            .assert()
            .code(1);
        assert!(output.get_output().stdout.is_empty());
        assert_eq!(problem(&output.get_output().stderr)["code"], "handle_taken");

        // A missing flag is clap's usage error, exit 2.
        zurfur(&url, home.path())
            .args(["account", "create", "--name", "No Handle"])
            .assert()
            .code(2);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn account_delete_over_the_binary() {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let url = db.url().to_string();

    let database = PgDatabase::new(pool);
    let mut uow = database.begin().await.expect("begin");
    uow.users()
        .provision(&Did::from(DELETE_DID.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    let home = tempfile::tempdir().unwrap();
    let identity_path = home.path().join(cli::identity::IDENTITY_FILE_NAME);
    cli::identity::save(
        &identity_path,
        &cli::identity::Identity::new(DELETE_DID, &url),
    )
    .unwrap();

    tokio::task::spawn_blocking(move || {
        let output = zurfur(&url, home.path())
            .args([
                "--json",
                "account",
                "create",
                "--name",
                "Doomed Studio",
                "--handle",
                "doomed.zurfur.app",
            ])
            .assert()
            .success();
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        let founded: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        let account_id = founded["id"].as_str().unwrap().to_string();

        // Without `--yes` and with no terminal to ask, the deletion is
        // REFUSED, not defaulted: exit 1, stdout silent (Engineer ruling
        // 2026-08-28). `write_stdin` makes stdin a pipe, which is what a
        // script or CI hands the binary.
        let output = zurfur(&url, home.path())
            .args(["account", "delete", &account_id])
            .write_stdin("")
            .assert()
            .code(1);
        assert!(output.get_output().stdout.is_empty());
        assert_eq!(
            problem(&output.get_output().stderr)["code"],
            "confirmation_required"
        );

        // Piping `y` is not a confirmation either — the answer channel isn't a
        // person. Same refusal.
        let output = zurfur(&url, home.path())
            .args(["account", "delete", &account_id])
            .write_stdin("y\n")
            .assert()
            .code(1);
        assert_eq!(
            problem(&output.get_output().stderr)["code"],
            "confirmation_required"
        );

        // Success WITH `--yes`: exit 0, one compact JSON line carrying which
        // deletion happened. A fresh account holds no account-anchored fact,
        // so it is hard-deleted — and this succeeding proves the two refusals
        // above deleted nothing.
        let output = zurfur(&url, home.path())
            .args(["--json", "account", "delete", &account_id, "--yes"])
            .assert()
            .success();
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        assert_eq!(stdout.lines().count(), 1, "one compact line: {stdout:?}");
        assert_eq!(stdout.trim(), r#"{"outcome":"hard"}"#);

        // The same id again, this time through the short spelling of the
        // flag: the account is gone, a domain problem, exit 1. `delete`
        // looks the account up before it weighs the caller's standing, so a
        // gone account answers `account_not_found` rather than borrowing the
        // refusal meant for a caller who has no role on a real one.
        let output = zurfur(&url, home.path())
            .args(["account", "delete", &account_id, "-y"])
            .assert()
            .code(1);
        assert!(output.get_output().stdout.is_empty());
        assert_eq!(
            problem(&output.get_output().stderr)["code"],
            "account_not_found"
        );
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn bug_reports_list_over_the_binary_escapes_a_hostile_row() {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let url = db.url().to_string();

    let database = PgDatabase::new(pool.clone());
    let mut uow = database.begin().await.expect("begin");
    uow.users()
        .provision(&Did::from(REPORTER_DID.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    // Raw SQL, past every domain rule: a C1 CSI and a tag character in the
    // text and in a header field.
    let report_id = uuid::Uuid::now_v7();
    let hostile = "csi \u{9b}[2J, tag \u{e0041}";
    sqlx::query(
        "INSERT INTO bug_report (id, reporter_id, description, browser, api_version, received_at)
         VALUES ($1, $2, $3, $3, 'api-1', now())",
    )
    .bind(report_id)
    .bind(REPORTER_DID)
    .bind(hostile)
    .execute(&pool)
    .await
    .expect("the hostile row is written");
    let id_text = report_id.to_string();
    let reference = id_text[id_text.len() - 8..].to_owned();
    // No identity file: an operator command needs none.
    let home = tempfile::tempdir().unwrap();

    tokio::task::spawn_blocking(move || {
        let output = zurfur(&url, home.path())
            .args(["bug-reports", "list"])
            .assert()
            .success();
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        assert!(!stdout.contains('\u{9b}'), "a raw C1 CSI: {stdout:?}");
        assert!(!stdout.contains('\u{e0041}'), "a raw tag: {stdout:?}");
        let escaped = "csi \\u{9b}[2J, tag \\u{e0041}";
        let expected_browser = format!("  browser: {escaped}");
        let expected_text = format!("    {escaped}");
        assert!(
            stdout.lines().any(|line| line == expected_browser),
            "{stdout}"
        );
        assert!(stdout.lines().any(|line| line == expected_text), "{stdout}");
        let expected_fence = format!("===== bug report {reference} =====");
        assert_eq!(stdout.lines().next(), Some(expected_fence.as_str()));

        // `--json`: printable ASCII that parses back to the stored text.
        let output = zurfur(&url, home.path())
            .args(["--json", "bug-reports", "list"])
            .assert()
            .success();
        let stdout = output.get_output().stdout.clone();
        assert!(
            stdout
                .iter()
                .all(|byte| (b' '..=b'~').contains(byte) || *byte == b'\n'),
            "non-ASCII JSON: {stdout:?}"
        );
        let listed: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(listed["reports"][0]["browser"], hostile);
        assert_eq!(listed["reports"][0]["description"], hostile);
        assert_eq!(listed["reports"][0]["reference"], reference.as_str());

        // `--since` after the row: the fixed empty line, never nothing.
        let output = zurfur(&url, home.path())
            .args(["bug-reports", "list", "--since", "2999-01-01T00:00:00Z"])
            .assert()
            .success();
        assert_eq!(output.get_output().stdout, b"no bug reports\n");

        // A `--since` that isn't RFC 3339 is clap's usage error, exit 2.
        zurfur(&url, home.path())
            .args(["bug-reports", "list", "--since", "2026-10-17"])
            .assert()
            .code(2);
    })
    .await
    .unwrap();
}
