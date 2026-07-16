//! The commission fact predicate over PostgreSQL (ZMVP-67), against a throwaway
//! container: `commission_has_facts` lives on the [`UnitOfWork`]'s commissions
//! view — the same transaction a future delete/archive gate (ZMVP-66/68) runs in
//! — and, with no fact-minter wired, every commission answers `false`. The
//! schema tripwire below is what forces a future fact-minter to wire its table
//! into the predicate **deliberately**. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::collections::BTreeSet;

use adapter_pg::{COMMISSION_FACT_TABLES, COMMISSION_NON_FACT_TABLES, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, CommissionTitle, NewChangelogEntry,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work (`commission.owner_id`
/// references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// AC2+AC3 (pg): a freshly inserted commission holds no facts — asked in the
/// **same transaction** that created it (the transactional read the delete gate
/// needs, so check-then-delete has no TOCTOU window), and again from a later
/// unit after the commit.
#[tokio::test]
async fn every_commission_answers_false_with_no_fact_minters_wired() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:factless-owner").await;
    let title = CommissionTitle::try_new("A ref sheet").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = commission.id;

    let db = PgDatabase::new(pool.clone());

    // Same-transaction read: create and ask inside one unit of work.
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.create(&commission).await.expect("create");
        let has_facts = commissions
            .commission_has_facts(id)
            .await
            .expect("has_facts in the creating unit");
        assert!(
            !has_facts,
            "no fact-minter exists, so no commission can bear facts"
        );
    }
    uow.commit().await.expect("commit");

    // A later unit of work sees the committed commission and the same answer.
    let mut uow = db.begin().await.expect("begin second unit");
    let has_facts = uow
        .commissions()
        .commission_has_facts(id)
        .await
        .expect("has_facts in a later unit");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// A commission id nobody ever created also answers `false`: absence of the
/// commission is absence of facts, not an error.
#[tokio::test]
async fn an_unknown_commission_answers_false() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    let has_facts = uow
        .commissions()
        .commission_has_facts(CommissionId::new(uuid::Uuid::now_v7()))
        .await
        .expect("has_facts for an unknown id");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// ZMVP-66 AC1 (pg): the hard delete — gated by `commission_has_facts` **in the
/// same transaction** (ruling E17) — removes the `commission` row, and every
/// child table in this lineage reaps via its `ON DELETE CASCADE` (ruling E35):
/// `commission_changelog` is the only commission-referencing table at this
/// stack. Also proves the delete is transactional: a rolled-back unit deletes
/// nothing.
#[tokio::test]
async fn hard_delete_reaps_the_row_and_cascades_the_changelog() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:deleting-owner").await;
    let title = CommissionTitle::try_new("Doomed").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = commission.id;

    let db = PgDatabase::new(pool.clone());

    // Arrange: the commission plus a changelog entry (its child row).
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().create(&commission).await.expect("create");
    uow.changelog()
        .append(&NewChangelogEntry::event(
            id,
            ChangelogEntryKind::Created,
            owner.id,
            serde_json::json!({ "title": "Doomed" }),
            Utc::now(),
        ))
        .await
        .expect("append genesis entry");
    uow.commit().await.expect("commit");

    let changelog_rows = |pool: PgPool| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM commission_changelog WHERE commission_id = $1",
        )
        .bind(*id)
        .fetch_one(&pool)
        .await
        .expect("count changelog rows")
    };
    assert_eq!(
        changelog_rows(pool.clone()).await,
        1,
        "the child row exists"
    );

    // A rolled-back delete removes nothing (the delete rides the transaction).
    let mut uow = db.begin().await.expect("begin rollback unit");
    uow.commissions().delete(id).await.expect("staged delete");
    uow.rollback().await.expect("rollback");
    assert_eq!(
        changelog_rows(pool.clone()).await,
        1,
        "a rolled-back delete leaves the child row"
    );

    // The real delete: gate and delete inside ONE unit of work (ruling E17).
    let mut uow = db.begin().await.expect("begin delete unit");
    {
        let mut commissions = uow.commissions();
        let has_facts = commissions
            .commission_has_facts(id)
            .await
            .expect("gate in the deleting unit");
        assert!(!has_facts, "fact-free by construction");
        commissions.delete(id).await.expect("delete");
    }
    uow.commit().await.expect("commit delete");

    let row_count: i64 =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commission WHERE id = $1")
            .bind(*id)
            .fetch_one(&pool)
            .await
            .expect("count commission rows");
    assert_eq!(row_count, 0, "the commission row is gone");
    assert_eq!(
        changelog_rows(pool.clone()).await,
        0,
        "commission_changelog cascaded away (ON DELETE CASCADE)"
    );
}

/// ZMVP-68 (pg store layer): `set_archived` flips the `archived_at` column in
/// the unit of work and reports **whether the state actually transitioned** —
/// the bool the route keys its changelog append on, so a repeated archive (or
/// un-archive) can never mint a duplicate entry. The first stamp survives a
/// repeat; both directions round-trip through the read store; an unknown id is
/// a no-op.
#[tokio::test]
async fn set_archived_round_trips_and_reports_transitions() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:archiving-owner").await;
    let commission = Commission::create(
        CommissionTitle::try_new("A ref sheet").expect("valid title"),
        owner.id,
        Utc::now(),
        None,
    );
    let id = commission.id;

    let db = PgDatabase::new(pool.clone());
    let store = adapter_pg::PgCommissionStore::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    uow.commissions().create(&commission).await.expect("create");
    uow.commit().await.expect("commit");
    assert!(
        store
            .find(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "a fresh commission is active"
    );

    // Archive: a real transition, visible after commit.
    let stamp = Utc::now();
    let mut uow = db.begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(id, Some(stamp))
        .await
        .expect("archive");
    assert!(changed, "active -> archived is a transition");
    uow.commit().await.expect("commit");
    let stored = store
        .find(id)
        .await
        .expect("find")
        .expect("the record survives archiving");
    let stored_stamp = stored.archived_at.expect("the flag is set");
    assert_eq!(
        stored_stamp.timestamp_micros(),
        stamp.timestamp_micros(),
        "the stamp round-trips (at the column's microsecond precision)",
    );
    assert_eq!(
        stored.title.as_str(),
        "A ref sheet",
        "the record survives intact"
    );

    // A repeat archive is no transition and keeps the first stamp.
    let mut uow = db.begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(id, Some(Utc::now()))
        .await
        .expect("repeat archive");
    assert!(!changed, "archived -> archived is not a transition");
    uow.commit().await.expect("commit");
    assert_eq!(
        store
            .find(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at,
        Some(stored_stamp),
        "a repeat archive never rewrites the original stamp",
    );

    // Un-archive: back to active; a repeat is no transition; unknown = no-op.
    let mut uow = db.begin().await.expect("begin");
    assert!(
        uow.commissions()
            .set_archived(id, None)
            .await
            .expect("unarchive"),
        "archived -> active is a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(id, None)
            .await
            .expect("repeat unarchive"),
        "active -> active is not a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(CommissionId::new(uuid::Uuid::now_v7()), Some(Utc::now()))
            .await
            .expect("archive an unknown id"),
        "an absent commission matches nothing (existence is the caller's check)",
    );
    uow.commit().await.expect("commit");
    assert!(
        store
            .find(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "un-archiving clears the flag"
    );
}

/// THE TRIPWIRE (ZMVP-67, conductor ruling E18): every table that references
/// `commission(id)` must be **deliberately classified** — either registered in
/// [`COMMISSION_FACT_TABLES`] (its rows are facts; the predicate must query it)
/// or exempted in [`COMMISSION_NON_FACT_TABLES`] (its rows are bookkeeping that
/// cascades away with the commission, e.g. a future changelog). A migration that
/// adds a commission-referencing table trips this test until its author makes
/// that call in code — and registering a fact table trips the compile-time guard
/// in `adapter_pg::commission`, which refuses to build until the constant-`false`
/// predicate is replaced by a real query. Neither step can happen by accident.
#[tokio::test]
async fn every_commission_referencing_table_is_classified_as_fact_or_non_fact() {
    let (pool, _container) = fresh_pool().await;

    // Every table holding a foreign key onto `commission` — the schema-level
    // superset of possible commission-anchored storage.
    let referencing: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT conrelid::regclass::text
        FROM pg_constraint
        WHERE contype = 'f' AND confrelid = 'commission'::regclass
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("scan foreign keys onto commission");

    let referencing: BTreeSet<&str> = referencing.iter().map(String::as_str).collect();
    let facts: BTreeSet<&str> = COMMISSION_FACT_TABLES.iter().copied().collect();
    let non_facts: BTreeSet<&str> = COMMISSION_NON_FACT_TABLES.iter().copied().collect();

    let overlap: Vec<&&str> = facts.intersection(&non_facts).collect();
    assert!(
        overlap.is_empty(),
        "a table cannot be both fact and non-fact: {overlap:?}"
    );

    let classified: BTreeSet<&str> = facts.union(&non_facts).copied().collect();
    assert_eq!(
        referencing, classified,
        "every table referencing commission(id) must be listed in exactly one of \
         COMMISSION_FACT_TABLES (the commission_has_facts registry, Deletion DD 3014657) \
         or COMMISSION_NON_FACT_TABLES (deliberate exemptions) in \
         adapter-pg/src/commission.rs — classify it there, in the same change that \
         adds the table"
    );
}
