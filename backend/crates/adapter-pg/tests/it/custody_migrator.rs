//! The custody migrator: it probes and dry-runs every 0.1 custody row before any
//! migration and refuses, naming a count, when one fails; it re-wraps the 0.1 rows
//! after the migrations, keeping their ciphertext; it reports its counts; `check`
//! writes nothing; the probe reads only `public.account_keys`. Requires a container
//! runtime socket (DOCKER_HOST honored).

use adapter_pg::{CustodyCheck, CustodyMigrationError, CustodyMigrator, PgKeyStore, PgPool};
use domain::elements::{custody::ForDidCustody, did::Did};
use domain::ports::{KeyStore, ScopedSealer};
use test_support::custody::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_DID, LEGACY_BOB_BLOB_HEX, LEGACY_BOB_DID, LEGACY_ROOT,
    hex_bytes, legacy_keys, lying_did_custody, test_custody,
};
use zeroize::Zeroizing;

use crate::migration_steps::{bare_pool, run_migrations_before};

/// The envelope's migration, as sqlx numbers it.
const CUSTODY_ENVELOPE: i64 = 20261022231354;

/// A root the legacy fixtures don't open under.
const WRONG_ROOT: [u8; 32] = [8u8; 32];

/// A migrator over `pool` with the 0.1 `root` and the DID-custody `sealer`.
fn migrator(pool: &PgPool, root: [u8; 32], sealer: ScopedSealer<ForDidCustody>) -> CustodyMigrator {
    CustodyMigrator::new(pool.clone(), Zeroizing::new(root), sealer)
}

/// A migrator with the fixtures' root and the test custodian.
fn honest_migrator(pool: &PgPool) -> CustodyMigrator {
    migrator(pool, LEGACY_ROOT, test_custody().did_custody)
}

/// Store one 0.1 row, naming only the columns the table had before the envelope.
async fn seed_legacy(pool: &PgPool, did: &str, wrapped_keys: &[u8]) {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, created_at)
         VALUES ($1, $2, now())",
    )
    .bind(did)
    .bind(wrapped_keys)
    .execute(pool)
    .await
    .expect("seed a 0.1 row");
}

/// A database on the schema before the envelope, holding Alice's and Bob's 0.1 rows.
async fn before_the_envelope_with_two_rows() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, CUSTODY_ENVELOPE).await;
    seed_legacy(&pool, LEGACY_ALICE_DID, &hex_bytes(LEGACY_ALICE_BLOB_HEX)).await;
    seed_legacy(&pool, LEGACY_BOB_DID, &hex_bytes(LEGACY_BOB_BLOB_HEX)).await;
    (pool, container)
}

/// Every version the ledger records, in order.
async fn ledger(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read the ledger")
}

/// The custody table's columns, in order.
async fn custody_columns(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'account_keys'
         ORDER BY ordinal_position",
    )
    .fetch_all(pool)
    .await
    .expect("read the custody columns")
}

/// Every custody row, whole, as text, by DID.
async fn custody_rows(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT to_jsonb(k)::text FROM account_keys k ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the custody rows")
}

/// Every row's two blobs, by DID.
async fn blobs(pool: &PgPool) -> Vec<(String, Option<Vec<u8>>, Option<Vec<u8>>)> {
    sqlx::query_as("SELECT did, wrapped_keys, sealed_keys FROM account_keys ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the blobs")
}

/// How many embedded migrations are at or after the envelope's.
fn migrations_from_the_envelope() -> usize {
    adapter_pg::migrator()
        .migrations
        .iter()
        .filter(|migration| migration.version >= CUSTODY_ENVELOPE)
        .count()
}

#[tokio::test]
async fn wrong_root_refuses_before_any_migration() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;
    let ledger_before = ledger(&pool).await;
    let columns_before = custody_columns(&pool).await;

    let run = migrator(&pool, WRONG_ROOT, test_custody().did_custody)
        .run()
        .await;

    let refusal = run.expect_err("a root the rows don't open under refuses");
    assert!(matches!(
        refusal,
        CustodyMigrationError::Unopenable { count: 2 }
    ));
    assert_eq!(
        refusal.to_string(),
        "2 custody rows don't open under the configured root"
    );
    assert_eq!(ledger(&pool).await, ledger_before, "no migration ran");
    assert_eq!(
        custody_columns(&pool).await,
        columns_before,
        "the columns are unchanged"
    );
}

#[tokio::test]
async fn probe_on_a_fresh_database_is_a_no_op() {
    let (pool, _container) = bare_pool().await;

    let check = honest_migrator(&pool)
        .check()
        .await
        .expect("check a bare database");
    let report = honest_migrator(&pool)
        .run()
        .await
        .expect("run on a bare database");

    let expected_check = CustodyCheck {
        unopenable: 0,
        pending: 0,
    };
    assert_eq!(check, expected_check, "no custody table, nothing to probe");
    let every_migration = adapter_pg::migrator().migrations.len();
    assert_eq!(report.applied, every_migration);
    assert_eq!(report.rows_rewrapped, 0);
}

#[tokio::test]
async fn a_self_check_failure_refuses_before_any_migration() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;
    let ledger_before = ledger(&pool).await;
    let columns_before = custody_columns(&pool).await;

    let check = migrator(&pool, LEGACY_ROOT, lying_did_custody())
        .check()
        .await
        .expect("check with a lying custodian");
    let run = migrator(&pool, LEGACY_ROOT, lying_did_custody())
        .run()
        .await;

    assert_eq!(check.unopenable, 2, "both rows fail the dry run");
    let refusal = run.expect_err("a dry run that opens to other keys refuses");
    assert!(matches!(
        refusal,
        CustodyMigrationError::Unopenable { count: 2 }
    ));
    assert_eq!(ledger(&pool).await, ledger_before, "no migration ran");
    assert_eq!(
        custody_columns(&pool).await,
        columns_before,
        "the columns are unchanged"
    );
}

#[tokio::test]
async fn rewrap_keeps_wrapped_keys_byte_identical() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;

    honest_migrator(&pool)
        .run()
        .await
        .expect("the migration runs");

    let stored = blobs(&pool).await;
    let alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);
    let bob_blob = hex_bytes(LEGACY_BOB_BLOB_HEX);
    let expected = [(LEGACY_ALICE_DID, alice_blob), (LEGACY_BOB_DID, bob_blob)];
    assert_eq!(stored.len(), expected.len());
    for ((did, wrapped_keys, sealed_keys), (expected_did, expected_blob)) in
        stored.iter().zip(expected.iter())
    {
        assert_eq!(did, expected_did);
        assert_eq!(
            wrapped_keys.as_ref(),
            Some(expected_blob),
            "{did} keeps its 0.1 ciphertext"
        );
        assert!(sealed_keys.is_some(), "{did} is sealed");
    }
}

#[tokio::test]
async fn rewrap_twice_changes_nothing() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;
    honest_migrator(&pool).run().await.expect("the first run");
    let after_first = blobs(&pool).await;

    let second = honest_migrator(&pool).run().await.expect("the second run");

    assert_eq!(second.rows_rewrapped, 0, "nothing is left to re-wrap");
    assert_eq!(
        blobs(&pool).await,
        after_first,
        "every envelope is kept as it was"
    );
}

#[tokio::test]
async fn migrate_reports_its_counts() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;

    let first = honest_migrator(&pool).run().await.expect("the first run");
    let second = honest_migrator(&pool).run().await.expect("the second run");

    let latest = adapter_pg::migrator().migrations.last().map(|m| m.version);
    assert_eq!(first.applied, migrations_from_the_envelope());
    assert_eq!(first.rows_rewrapped, 2);
    assert_eq!(first.version, latest);
    assert_eq!((second.applied, second.rows_rewrapped), (0, 0));
    let store = PgKeyStore::new(pool.clone(), test_custody().did_custody);
    for did in [LEGACY_ALICE_DID, LEGACY_BOB_DID] {
        let keys = store
            .get(&Did::from(did.to_owned()))
            .await
            .expect("open the envelope");
        assert_eq!(
            keys,
            Some(legacy_keys()),
            "{did}'s envelope opens to its 0.1 keys"
        );
    }
}

#[tokio::test]
async fn check_writes_nothing() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;
    let rows_before = custody_rows(&pool).await;
    let ledger_before = ledger(&pool).await;
    let columns_before = custody_columns(&pool).await;

    let check = honest_migrator(&pool).check().await.expect("check");

    let expected = CustodyCheck {
        unopenable: 0,
        pending: 2,
    };
    assert_eq!(check, expected, "both rows open, both await a re-wrap");
    assert_eq!(
        custody_rows(&pool).await,
        rows_before,
        "the rows are unchanged"
    );
    assert_eq!(ledger(&pool).await, ledger_before, "no migration ran");
    assert_eq!(
        custody_columns(&pool).await,
        columns_before,
        "the columns are unchanged"
    );
}

#[tokio::test]
async fn check_counts_the_unsealed_rows_after_the_envelope() {
    let (pool, _container) = before_the_envelope_with_two_rows().await;
    adapter_pg::migrate_fresh(&pool)
        .await
        .expect("migrate without re-wrapping");

    let check = honest_migrator(&pool).check().await.expect("check");

    let expected = CustodyCheck {
        unopenable: 0,
        pending: 2,
    };
    assert_eq!(check, expected);
}

#[tokio::test]
async fn the_probe_checks_public_account_keys() {
    let db = test_support::pg::bare_db().await;
    let setup = adapter_pg::connect(db.url()).await.expect("connect");
    sqlx::raw_sql(
        "CREATE SCHEMA elsewhere;
         CREATE TABLE elsewhere.account_keys (did text PRIMARY KEY, wrapped_keys bytea NOT NULL);
         INSERT INTO elsewhere.account_keys VALUES ('did:plc:shadow', decode('00', 'hex'));
         DO $$ BEGIN
             EXECUTE format('ALTER DATABASE %I SET search_path = elsewhere, public', current_database());
         END $$;",
    )
    .execute(&setup)
    .await
    .expect("shadow the custody table outside public");
    setup.close().await;
    let pool = adapter_pg::connect(db.url()).await.expect("reconnect");
    let shadowed: bool = sqlx::query_scalar("SELECT to_regclass('account_keys') IS NOT NULL")
        .fetch_one(&pool)
        .await
        .expect("resolve the bare name");
    assert!(
        shadowed,
        "the bare name resolves to the table outside public"
    );

    let check = honest_migrator(&pool).check().await.expect("check");

    let expected = CustodyCheck {
        unopenable: 0,
        pending: 0,
    };
    assert_eq!(check, expected, "only public.account_keys is probed");
}

#[tokio::test]
async fn debug_prints_no_key_bytes() {
    let root = [0x5Au8; 32];
    let pool = adapter_pg::lazy_pool("postgres://nobody@127.0.0.1:1/none").expect("a lazy pool");
    let store = PgKeyStore::new(pool.clone(), test_custody().did_custody);
    let migrator = migrator(&pool, root, test_custody().did_custody);

    let shown = [format!("{store:?}"), format!("{migrator:?}")];

    let hex = "5a5a";
    let base64 = "WlpaWlpa";
    let decimal = "90, 90";
    for text in &shown {
        for form in [hex, &hex.to_uppercase(), base64, decimal] {
            assert!(!text.contains(form), "{text} holds the root as {form}");
        }
    }
}
