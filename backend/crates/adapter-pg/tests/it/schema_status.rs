//! `schema_status` against a real Postgres (ZMVP-206): bare → Unknown,
//! partially migrated → Behind{pending}, fully migrated → Current, a ledger
//! with versions this binary never embedded → Ahead{unknown}.

use adapter_pg::{SchemaStatus, migrator, schema_status};

#[tokio::test]
async fn bare_then_partial_then_current() {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.unwrap();

    assert_eq!(schema_status(&pool).await.unwrap(), SchemaStatus::Unknown);

    // Apply only the first migration by targeting its version.
    let migrator = migrator();
    let first = migrator
        .iter()
        .next()
        .expect("at least one migration")
        .version;
    let total = migrator.iter().count();
    migrator.run_to(first, &pool).await.unwrap();
    let status = schema_status(&pool).await.unwrap();
    if total > 1 {
        assert_eq!(status, SchemaStatus::Behind { pending: total - 1 });
    } else {
        assert_eq!(status, SchemaStatus::Current);
    }

    adapter_pg::migrate(&pool).await.unwrap();
    assert_eq!(schema_status(&pool).await.unwrap(), SchemaStatus::Current);
}

#[tokio::test]
async fn a_newer_binarys_ledger_reads_as_ahead() {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.unwrap();
    adapter_pg::migrate(&pool).await.unwrap();

    // A version far past anything embedded, as a newer build would record.
    assert_eq!(migrator().table_name, "_sqlx_migrations");
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES ($1, 'from the future', true, '\\x00'::bytea, 0)",
    )
    .bind(i64::MAX)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        schema_status(&pool).await.unwrap(),
        SchemaStatus::Ahead { unknown: 1 }
    );
}
