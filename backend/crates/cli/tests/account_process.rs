//! `zurfur account create` end to end against a real throwaway Postgres:
//! clap's subcommand + flag spellings, the stdout JSON, and the exit classes
//! — the process-level counterpart of `tests/account.rs`.

mod common;
use adapter_pg::PgDatabase;
use assert_cmd::Command;
use common::problem;
use domain::elements::did::Did;
use domain::ports::Database;

const DID: &str = "did:plc:cli-account-process";

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
        .provision(&Did::new(DID.to_string()))
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
