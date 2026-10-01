use super::{DEADLINE_SWEEP_LOCK_KEY, sweep_pass_as_leader};

// Finding 5: the sweep pass is single-writer. While another connection holds the
// sweeper's advisory lock, a pass must SKIP (`None`); once the lock frees, the pass
// runs (`Some`). Deterministic — `pg_try_advisory_xact_lock` is a non-blocking
// try-lock — and an empty DB means the pass that runs marks zero. This is the guard
// that stops two api instances both appending a Late entry for the same lapse.
#[tokio::test]
async fn a_pass_skips_while_another_instance_holds_the_sweeper_lock() {
    let db = test_support::pg::fresh_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    let database = adapter_pg::PgDatabase::new(pool.clone());

    // Stand in for another instance mid-sweep: hold the xact-scoped lock open on a
    // separate connection (released only when this transaction ends).
    let mut holder = pool.begin().await.expect("begin holder txn");
    let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(DEADLINE_SWEEP_LOCK_KEY)
        .fetch_one(&mut *holder)
        .await
        .expect("holder takes the lock");
    assert!(held, "the holder acquires the sweeper lock");

    // A pass now finds the lock taken and skips — no second writer.
    let skipped = sweep_pass_as_leader(&database, &pool)
        .await
        .expect("pass runs without error");
    assert_eq!(skipped, None, "a pass skips while another holds the lock");

    // Release the lock; the next pass wins it and runs (empty DB → marks zero).
    holder.rollback().await.expect("release the lock");
    let ran = sweep_pass_as_leader(&database, &pool)
        .await
        .expect("pass runs without error");
    assert_eq!(ran, Some(0), "once the lock frees, the pass runs");
}
