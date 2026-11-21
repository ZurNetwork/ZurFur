//! `zurfur account create` / `delete` end to end against a real throwaway
//! Postgres: clap's subcommand + flag spellings, the stdout JSON, and the exit
//! classes — the process-level counterpart of `tests/account.rs`. Also the
//! custody root check `account create` runs, and the operator's `bug-reports
//! list` over a hostile row written past every rule.

mod common;
use adapter_pg::PgDatabase;
use assert_cmd::Command;
use common::problem;
use domain::elements::did::Did;
use domain::ports::Database;

const DID: &str = "did:plc:cli-account-process";
const DELETE_DID: &str = "did:plc:cli-account-delete-process";
const REPORTER_DID: &str = "did:plc:cli-bug-reports-process";
const ROOT_CHECK_DID: &str = "did:plc:cli-account-root-check";

/// A valid 32-byte base64 root other than [`common::TEST_ROOT_KEY`].
const OTHER_ROOT_KEY: &str = "ERERERERERERERERERERERERERERERERERERERERERE=";

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

/// Every Account's id and every custody row, by key.
type AccountAndCustodyRows = (Vec<String>, Vec<(String, Option<Vec<u8>>)>);

/// A database with `ROOT_CHECK_DID` provisioned, and a home whose identity file acts
/// as it.
async fn root_check_world() -> (
    adapter_pg::PgPool,
    test_support::pg::TestDb,
    tempfile::TempDir,
) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let mut uow = database.begin().await.expect("begin");
    uow.users()
        .provision(&Did::from(ROOT_CHECK_DID.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    let home = tempfile::tempdir().unwrap();
    let identity_path = home.path().join(cli::identity::IDENTITY_FILE_NAME);
    let identity = cli::identity::Identity::new(ROOT_CHECK_DID, db.url());
    cli::identity::save(&identity_path, &identity).unwrap();
    (pool, db, home)
}

/// `account create` of `handle` with `root` in the custody root's variable, run off
/// the async runtime.
async fn create_account_under(
    url: &str,
    home: &std::path::Path,
    root: &str,
    handle: &str,
) -> std::process::Output {
    let mut command = zurfur(url, home);
    command.env(composition::ROOT_KEY_ENV, root).args([
        "--json",
        "account",
        "create",
        "--name",
        "Root Check Studio",
        "--handle",
        handle,
    ]);
    tokio::task::spawn_blocking(move || command.output().expect("the binary runs"))
        .await
        .unwrap()
}

/// Every Account's id and every custody row.
async fn account_and_custody_rows(pool: &adapter_pg::PgPool) -> AccountAndCustodyRows {
    let accounts: Vec<String> = sqlx::query_scalar("SELECT id FROM accounts ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("read the accounts");
    let custody: Vec<(String, Option<Vec<u8>>)> =
        sqlx::query_as("SELECT did, sealed_keys FROM account_keys ORDER BY did")
            .fetch_all(pool)
            .await
            .expect("read the custody rows");
    (accounts, custody)
}

#[tokio::test]
async fn account_create_refuses_a_root_that_doesnt_open_the_custody_rows() {
    let (pool, db, home) = root_check_world().await;
    let url = db.url().to_string();
    let founded =
        create_account_under(&url, home.path(), common::TEST_ROOT_KEY, "first.zurfur.app").await;
    assert!(founded.status.success(), "the right root founds an Account");
    let before = account_and_custody_rows(&pool).await;

    let refused =
        create_account_under(&url, home.path(), OTHER_ROOT_KEY, "second.zurfur.app").await;

    assert_eq!(refused.status.code(), Some(3), "a config refusal is exit 3");
    assert!(refused.stdout.is_empty());
    let refusal = problem(&refused.stderr);
    assert_eq!(refusal["code"], "config");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains(composition::ROOT_KEY_ENV), "{stderr}");
    assert!(
        !stderr.contains("did:"),
        "the refusal names no DID: {stderr}"
    );
    let after = account_and_custody_rows(&pool).await;
    assert_eq!(after, before, "the refused command writes nothing");

    // A command holding no purpose reads no root and runs no root check.
    let mut rootless = zurfur(&url, home.path());
    rootless
        .env_remove(composition::ROOT_KEY_ENV)
        .args(["bug-reports", "list"]);
    let listed = tokio::task::spawn_blocking(move || rootless.output().expect("the binary runs"))
        .await
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
}

#[tokio::test]
async fn account_create_runs_under_the_root_the_custody_rows_open_under() {
    let (pool, db, home) = root_check_world().await;
    let url = db.url().to_string();
    let first =
        create_account_under(&url, home.path(), common::TEST_ROOT_KEY, "first.zurfur.app").await;
    assert!(first.status.success(), "the first Account is founded");

    let second = create_account_under(
        &url,
        home.path(),
        common::TEST_ROOT_KEY,
        "second.zurfur.app",
    )
    .await;

    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let (accounts, custody) = account_and_custody_rows(&pool).await;
    assert_eq!(accounts.len(), 2);
    assert_eq!(custody.len(), 2);
}

#[tokio::test]
async fn account_create_runs_under_any_root_without_a_sealed_custody_row() {
    let (pool, db, home) = root_check_world().await;
    let url = db.url().to_string();

    let founded = create_account_under(&url, home.path(), OTHER_ROOT_KEY, "first.zurfur.app").await;

    assert!(
        founded.status.success(),
        "{}",
        String::from_utf8_lossy(&founded.stderr)
    );
    let (accounts, custody) = account_and_custody_rows(&pool).await;
    assert_eq!(accounts.len(), 1);
    assert_eq!(custody.len(), 1);
}
