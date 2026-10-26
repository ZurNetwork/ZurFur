//! The commission create over PostgreSQL: it writes only the starting state, bound to the
//! gate's node, and a node that isn't the commission's own fails at the statement. Then the
//! commission fact predicate, against a throwaway
//! container: `commission_has_facts` lives on the [`UnitOfWork`]'s commissions
//! view — the same transaction a future delete/archive gate runs in
//! — and, with no fact-minter wired, every commission answers `false`. The
//! schema tripwire below is what forces a future fact-minter to wire its table
//! into the predicate **deliberately**. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::collections::BTreeSet;

use adapter_pg::{COMMISSION_FACT_TABLES, COMMISSION_NON_FACT_TABLES, PgDatabase, PgPool};
use chrono::{SubsecRound, Utc};
use domain::{
    elements::{
        commission::{
            CommissionBirth, CommissionId, CommissionStart, NewChangelogEntry, NewCommission,
        },
        did::Did,
        level::Level,
        maturity::{Maturity, MaturityRating},
        node::{NewNode, NodeId, NodeType},
        user::User,
    },
    ports::{CommissionStore, Database, UnitOfWork},
};
use test_support::seeder::{commission_node, entry_nodes, write_commission};

use crate::seed::{self, MintingSeeder, seed_titled};

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
        .provision(&Did::from(did.to_string()))
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
    let db = PgDatabase::new(pool.clone());

    // Same-transaction read: create and ask inside one unit of work.
    let mut uow = db.begin().await.expect("begin");
    let id = write_commission(
        &mut *uow,
        &MintingSeeder,
        &owner.id,
        Level::Private,
        "A ref sheet",
    )
    .await;
    let has_facts = uow
        .commissions()
        .commission_has_facts(&id)
        .await
        .expect("has_facts in the creating unit");
    assert!(
        !has_facts,
        "no fact-minter exists, so no commission can bear facts"
    );
    uow.commit().await.expect("commit");

    // A later unit of work sees the committed commission and the same answer.
    let mut uow = db.begin().await.expect("begin second unit");
    let has_facts = uow
        .commissions()
        .commission_has_facts(&id)
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
        .commission_has_facts(&CommissionId::from(uuid::Uuid::now_v7()))
        .await
        .expect("has_facts for an unknown id");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// The hard delete — gated by `commission_has_facts` **in the
/// same transaction** — removes the `commission` row, and every
/// child table in this lineage reaps via its `ON DELETE CASCADE`:
/// `commission_changelog` is the only commission-referencing table at this
/// stack. Also proves the delete is transactional: a rolled-back unit deletes
/// nothing.
#[tokio::test]
async fn hard_delete_reaps_the_row_and_cascades_the_changelog() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:deleting-owner").await;
    let db = PgDatabase::new(pool.clone());

    // Arrange: the commission plus a changelog entry (its child row).
    let mut uow = db.begin().await.expect("begin");
    let id = write_commission(
        &mut *uow,
        &MintingSeeder,
        &owner.id,
        Level::Private,
        "Doomed",
    )
    .await;
    uow.changelog()
        .append(&NewChangelogEntry::created(id, owner.id, Utc::now()))
        .await
        .expect("append genesis entry");
    uow.commit().await.expect("commit");

    let changelog_rows = |pool: PgPool| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM commission_changelog WHERE commission_id = $1",
        )
        .bind(uuid::Uuid::from(id))
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
    uow.commissions().delete(&id).await.expect("staged delete");
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
            .commission_has_facts(&id)
            .await
            .expect("gate in the deleting unit");
        assert!(!has_facts, "fact-free by construction");
        commissions.delete(&id).await.expect("delete");
    }
    uow.commit().await.expect("commit delete");

    let row_count: i64 =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM commission WHERE id = $1")
            .bind(uuid::Uuid::from(id))
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

/// At the pg store layer, `set_archived` flips the `archived_at` column in
/// the unit of work and reports **whether the state actually transitioned** —
/// the bool the route keys its changelog append on, so a repeated archive (or
/// un-archive) can never mint a duplicate entry. The first stamp survives a
/// repeat; both directions round-trip through the read store; an unknown id is
/// a no-op.
#[tokio::test]
async fn set_archived_round_trips_and_reports_transitions() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:archiving-owner").await;
    let id = seed_titled(&pool, &owner.id, "A ref sheet").await;

    let db = PgDatabase::new(pool.clone());
    let store = adapter_pg::PgCommissionStore::new(pool.clone());
    assert!(
        store
            .find(&id)
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
        .set_archived(&id, Some(stamp))
        .await
        .expect("archive");
    assert!(changed, "active -> archived is a transition");
    uow.commit().await.expect("commit");
    let stored = store
        .find(&id)
        .await
        .expect("find")
        .expect("the record survives archiving");
    let summary = store
        .find_summary(&id)
        .await
        .expect("find_summary")
        .expect("the record survives archiving");
    let stored_stamp = stored.archived_at.expect("the flag is set");
    assert_eq!(
        stored_stamp.timestamp_micros(),
        stamp.timestamp_micros(),
        "the stamp round-trips (at the column's microsecond precision)",
    );
    assert_eq!(
        summary.title.as_str(),
        "A ref sheet",
        "the record survives intact"
    );

    // A repeat archive is no transition and keeps the first stamp.
    let mut uow = db.begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(&id, Some(Utc::now()))
        .await
        .expect("repeat archive");
    assert!(!changed, "archived -> archived is not a transition");
    uow.commit().await.expect("commit");
    assert_eq!(
        store
            .find(&id)
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
            .set_archived(&id, None)
            .await
            .expect("unarchive"),
        "archived -> active is a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(&id, None)
            .await
            .expect("repeat unarchive"),
        "active -> active is not a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(&CommissionId::from(uuid::Uuid::now_v7()), Some(Utc::now()))
            .await
            .expect("archive an unknown id"),
        "an absent commission matches nothing (existence is the caller's check)",
    );
    uow.commit().await.expect("commit");
    assert!(
        store
            .find(&id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "un-archiving clears the flag"
    );
}

/// THE TRIPWIRE: every table that references
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

// --- ZMVP-157: `list_owned_by` against real SQL. ---
//
// `WHERE owner_id = $1 AND archived_at IS NULL` scopes the owner's active
// listing. These tests prove the owner scoping and the active filter hold in
// postgres.

/// Create a commission owned by `owner` in its own committed unit of work.
async fn create_commission(pool: &PgPool, owner: &User, title: &str) -> CommissionId {
    seed_titled(pool, &owner.id, title).await
}

// The listing is owner-scoped and active-only: another user's commission never
// appears (the containment property an owner's listing rests on), and an
// archived one of the caller's own is filtered out — the active-view filtering
// `Commission::archived_at` documents as belonging to listing projections.
#[tokio::test]
async fn list_owned_by_returns_only_the_callers_active_commissions_in_id_order() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:pglist-comm-owner").await;
    let stranger = provision(&pool, "did:plc:pglist-comm-stranger").await;
    let store = adapter_pg::PgCommissionStore::new(pool.clone());

    let first = create_commission(&pool, &owner, "A ref sheet").await;
    let second = create_commission(&pool, &owner, "A full illustration").await;
    let archived = create_commission(&pool, &owner, "An old job").await;
    let theirs = create_commission(&pool, &stranger, "Someone else's job").await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_archived(&archived, Some(Utc::now()))
        .await
        .expect("archive");
    uow.commit().await.expect("commit");

    let rows = store.list_owned_by(&owner.id).await.expect("list_owned_by");
    let listed: Vec<CommissionId> = rows.iter().map(|commission| commission.id).collect();

    assert!(
        !listed.contains(&theirs),
        "another user's commission is never listed, got {listed:?}"
    );
    assert!(
        !listed.contains(&archived),
        "an archived commission is filtered out of the active listing, got {listed:?}"
    );
    assert!(
        listed.contains(&first) && listed.contains(&second),
        "both of the caller's active commissions are listed, got {listed:?}"
    );
    assert_eq!(
        listed.len(),
        2,
        "exactly the caller's two active commissions, got {listed:?}"
    );

    // Ordering is `ORDER BY id`, and commission ids are UUIDv7 — creation order.
    let order: Vec<uuid::Uuid> = listed.iter().map(|id| uuid::Uuid::from(*id)).collect();
    let mut expected = order.clone();
    expected.sort();
    assert_eq!(order, expected, "rows come back ascending by commission id");

    let owners: Vec<_> = rows.iter().map(|commission| &commission.owner_id).collect();
    assert!(
        owners.iter().all(|owner_id| **owner_id == owner.id),
        "rows are hydrated, not just ids: {owners:?}"
    );
}

// A user with nothing owns nothing: the empty listing is a plain empty vec, not
// an error — what an owner's listing answers for a new account.
#[tokio::test]
async fn list_owned_by_is_empty_for_a_user_with_no_commissions() {
    let (pool, _container) = fresh_pool().await;
    let newcomer = provision(&pool, "did:plc:pglist-comm-newcomer").await;
    let store = adapter_pg::PgCommissionStore::new(pool.clone());

    let rows = store
        .list_owned_by(&newcomer.id)
        .await
        .expect("list_owned_by");
    assert!(rows.is_empty(), "a user with no commissions lists none");
}

/// Writes `nodes` on `unit`, in order.
async fn write_nodes(unit: &mut dyn UnitOfWork, nodes: &[NewNode]) {
    let mut repo = unit.nodes();
    for node in nodes {
        repo.create(node).await.expect("store a node");
    }
}

/// A starting state chosen with no maturity and no deadline.
fn unrated() -> NewCommission {
    NewCommission {
        maturity: None,
        deadline: None,
    }
}

// A create stores the domain's starting state and nothing else. Its title and level are its
// node's; only the maturity and deadline are chosen.
#[tokio::test]
async fn a_created_commission_is_stored_in_the_domain_s_starting_state() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:pg-born-owner").await;
    let id = CommissionId::mint();
    let node = commission_node(id, Level::Listed, "A moonlit piece");
    let mut nodes = vec![node.clone()];
    nodes.extend(entry_nodes(node.id));
    // The columns keep microseconds, so the chosen instants are rounded to them.
    let created_at = Utc::now().trunc_subsecs(6);
    let deadline = created_at + chrono::Duration::days(30);
    let birth = CommissionBirth {
        node: node.id,
        owner_id: owner.id.clone(),
        created_at,
    };
    let maturity = Maturity {
        rating: MaturityRating::Suggestive,
        graphic: false,
    };
    let chosen = NewCommission {
        maturity: Some(maturity),
        deadline: Some(deadline.into()),
    };

    let db = PgDatabase::new(pool.clone());
    let mut unit = db.begin().await.expect("begin");
    write_nodes(&mut *unit, &nodes).await;
    seed::create(&mut *unit, id, &birth, &chosen)
        .await
        .expect("create");
    unit.commit().await.expect("commit");

    let store = adapter_pg::PgCommissionStore::new(pool.clone());
    let found = store
        .find(&id)
        .await
        .expect("find")
        .expect("the commission is stored");
    let stored_start = CommissionStart {
        lifecycle_step: found.lifecycle_step.clone(),
        dispute: found.dispute.clone(),
        direction_status: found.direction_status,
        delayed_flag: found.delayed_flag,
        linked_channel: found.linked_channel.clone(),
        archived_at: found.archived_at,
    };
    assert_eq!(
        stored_start,
        CommissionStart::default(),
        "the domain's starting state"
    );
    assert_eq!(found.maturity, Some(maturity), "the chosen maturity");
    assert_eq!(found.deadline, Some(deadline), "the chosen deadline");
    assert_eq!(found.owner_id, owner.id, "the gate's owner");
    assert_eq!(found.created_at, created_at, "the gate's time");
    let summary = store
        .find_summary(&id)
        .await
        .expect("find_summary")
        .expect("the commission is stored");
    assert_eq!(summary.title.as_str(), "A moonlit piece", "the node's name");
    assert_eq!(summary.visibility, Level::Listed, "the node's level");
    let node_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
            .bind(uuid::Uuid::from(id))
            .fetch_one(&pool)
            .await
            .expect("read the bound node");
    assert_eq!(
        node_id,
        Some(uuid::Uuid::from(node.id)),
        "bound to its node"
    );
}

// A create at a node that isn't the commission's own fails at the create's own statement,
// before any commit, and ends the unit: another commission's node, one of its own entries,
// and a top-level commission node whose segment isn't the id.
#[tokio::test]
async fn create_with_a_node_that_is_not_its_own_fails_at_the_statement() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:pg-misbound-owner").await;
    let other = seed_titled(&pool, &owner.id, "Another piece").await;
    let other_node: uuid::Uuid = sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
        .bind(uuid::Uuid::from(other))
        .fetch_one(&pool)
        .await
        .expect("the other commission's node");
    let id = CommissionId::mint();
    let own = commission_node(id, Level::Private, "Its own");
    let entries = entry_nodes(own.id);
    let stray = commission_node(CommissionId::mint(), Level::Private, "A stray");
    let brief = entries
        .iter()
        .find(|entry| entry.node_type == NodeType::CommissionBrief)
        .expect("the brief entry")
        .id;
    let mut nodes = vec![own, stray.clone()];
    nodes.extend(entries);
    let wrong_nodes = [
        ("another commission's node", NodeId::from(other_node)),
        ("an entry node", brief),
        ("a node whose segment isn't the id", stray.id),
    ];

    let db = PgDatabase::new(pool.clone());
    for (what, wrong) in wrong_nodes {
        let mut unit = db.begin().await.expect("begin");
        write_nodes(&mut *unit, &nodes).await;
        let birth = CommissionBirth {
            node: wrong,
            owner_id: owner.id.clone(),
            created_at: Utc::now(),
        };
        let created = seed::create(&mut *unit, id, &birth, &unrated()).await;
        assert!(created.is_err(), "{what} fails at the create");
        let committed = unit.commit().await;
        assert!(
            committed.is_err(),
            "{what}: the refused create ends the unit"
        );
    }
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM commission WHERE id = $1")
        .bind(uuid::Uuid::from(id))
        .fetch_one(&pool)
        .await
        .expect("count the commission");
    assert_eq!(stored, 0, "no commission is stored");
    let nodes: i64 = sqlx::query_scalar("SELECT count(*) FROM node WHERE segment = $1")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .expect("count its node");
    assert_eq!(nodes, 0, "none of the ended units' nodes is kept");
}
