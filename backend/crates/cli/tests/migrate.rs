//! `zurfur migrate` + the schema-drift gate end to end, on a
//! **bare** throwaway Postgres: data commands refuse until `migrate` runs;
//! `health` reports instead of refusing; `migrate` is idempotent. Over the 0.1
//! custody fixtures: `migrate` re-wraps and reports its counts, `--check` counts and
//! writes nothing, and the gate refuses while a row awaits its re-wrap.

mod common;
use adapter_pg::PgPool;
use assert_cmd::Command;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use common::problem as json;
use test_support::custody::{self, CUSTODY_ENVELOPE, LEGACY_ROOT};
use test_support::pg::TestDb;

/// A root the legacy fixtures don't open under.
const WRONG_ROOT: [u8; 32] = [8u8; 32];

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

/// `zurfur --json <args>` against `url` with `root` as the custody root and a fresh
/// identity home, run off the async runtime.
async fn zurfur_json(
    url: &str,
    root: [u8; 32],
    args: &'static [&'static str],
) -> std::process::Output {
    let url = url.to_owned();
    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().unwrap();
        zurfur(&url, home.path())
            .env(composition::ROOT_KEY_ENV, STANDARD.encode(root))
            .arg("--json")
            .args(args)
            .output()
            .expect("zurfur runs")
    })
    .await
    .unwrap()
}

/// A database on the schema before the envelope, holding Alice's and Bob's 0.1 rows.
async fn before_the_envelope_with_two_rows() -> (PgPool, TestDb) {
    let (pool, db) = custody::before_the_envelope().await;
    custody::seed_alice_and_bob(&pool).await;
    (pool, db)
}

/// The text a spawned run left on stderr.
fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[tokio::test]
async fn migrate_check_writes_nothing() {
    let (pool, db) = before_the_envelope_with_two_rows().await;
    let before = custody::store_snapshot(&pool).await;

    let output = zurfur_json(db.url(), LEGACY_ROOT, &["migrate", "--check"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let expected_counts = serde_json::json!({ "unopenable": 0, "pending": 2 });
    assert_eq!(json(&output.stdout), expected_counts);
    assert_eq!(
        custody::store_snapshot(&pool).await,
        before,
        "the check wrote nothing"
    );

    let output = zurfur_json(db.url(), WRONG_ROOT, &["migrate", "--check"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let expected_counts = serde_json::json!({ "unopenable": 2, "pending": 2 });
    assert_eq!(json(&output.stdout), expected_counts);
    assert_eq!(
        custody::store_snapshot(&pool).await,
        before,
        "the check wrote nothing"
    );
}

#[tokio::test]
async fn a_wrong_root_refuses_the_migration_by_count() {
    let (pool, db) = before_the_envelope_with_two_rows().await;
    let before = custody::store_snapshot(&pool).await;

    let output = zurfur_json(db.url(), WRONG_ROOT, &["migrate"]).await;

    assert_eq!(output.status.code(), Some(3), "{}", stderr_of(&output));
    let problem = json(&output.stderr);
    assert_eq!(problem["code"], "config");
    assert_eq!(
        problem["detail"],
        "migration refused: 2 custody rows don't open under the configured root"
    );
    assert!(
        !stderr_of(&output).contains("did:"),
        "{}",
        stderr_of(&output)
    );
    assert_eq!(custody::store_snapshot(&pool).await, before, "nothing ran");
}

#[tokio::test]
async fn migrate_reports_its_counts() {
    let (pool, db) = before_the_envelope_with_two_rows().await;
    let embedded = adapter_pg::migrator();
    let from_the_envelope = embedded
        .migrations
        .iter()
        .filter(|migration| migration.version >= CUSTODY_ENVELOPE)
        .count();
    let latest = embedded
        .migrations
        .last()
        .map(|migration| migration.version);

    let output = zurfur_json(db.url(), LEGACY_ROOT, &["migrate"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let first_report = serde_json::json!({
        "applied": from_the_envelope,
        "version": latest,
        "rowsRewrapped": 2,
    });
    assert_eq!(json(&output.stdout), first_report);

    let output = zurfur_json(db.url(), LEGACY_ROOT, &["migrate"]).await;

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let second_report = serde_json::json!({
        "applied": 0,
        "version": latest,
        "rowsRewrapped": 0,
    });
    assert_eq!(json(&output.stdout), second_report);
    let unsealed = adapter_pg::unsealed_custody_rows(&pool)
        .await
        .expect("count");
    assert_eq!(unsealed, 0);
}

#[tokio::test]
async fn unrewrapped_custody_refuses_until_migrate() {
    let (pool, db) = before_the_envelope_with_two_rows().await;
    let unsealed = adapter_pg::unsealed_custody_rows(&pool)
        .await
        .expect("count");
    assert_eq!(unsealed, 0, "no envelope column yet: nothing to count");
    // The schema catches up past the envelope without the re-wrap.
    adapter_pg::migrate_fresh(&pool)
        .await
        .expect("the remaining migrations run");
    let unsealed = adapter_pg::unsealed_custody_rows(&pool)
        .await
        .expect("count");
    assert_eq!(unsealed, 2);
    let create: &'static [&'static str] = &[
        "account",
        "create",
        "--name",
        "Never Founded",
        "--handle",
        "never-founded.zurfur.app",
    ];

    let output = zurfur_json(db.url(), LEGACY_ROOT, create).await;

    assert_eq!(output.status.code(), Some(3), "{}", stderr_of(&output));
    let problem = json(&output.stderr);
    assert_eq!(problem["code"], "service_unavailable");
    assert_eq!(
        problem["detail"],
        "2 custody row(s) await their re-wrap — run `zurfur migrate`"
    );
    assert!(
        !stderr_of(&output).contains("did:"),
        "{}",
        stderr_of(&output)
    );

    let output = zurfur_json(db.url(), LEGACY_ROOT, &["migrate"]).await;
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));

    let output = zurfur_json(db.url(), LEGACY_ROOT, create).await;

    // Past the gate: the next problem is the missing identity.
    assert_eq!(output.status.code(), Some(1), "{}", stderr_of(&output));
    assert_eq!(json(&output.stderr)["code"], "not_authenticated");
}
