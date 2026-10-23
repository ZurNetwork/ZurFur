//! The commission standing reads over PostgreSQL: the shared suite
//! (`test_support::commission_standing_contract`), an unreadable visibility read as Private,
//! the pool read's one statement for a stranger and a made-up id, that statement's node probe
//! running for both, and the locked read holding the commission row until its unit ends.
//! Requires a container runtime socket.

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{CommissionId, CommissionRole, CommissionStanding},
    did::Did,
    level::Level,
    user::UserId,
};
use domain::ports::{CommissionStore, Database};
use test_support::commission_standing_contract::ParticipantRows;
use test_support::pg_app::{StatementLog, pg_runtime_recording};
use test_support::seeder::{self, CommissionSeeder};

use crate::commission_reads::{allow_any_node_level, assert_the_node_probe_runs, store_node_level};
use crate::seed::MintingSeeder;

/// The shipped pool standing statement, as the generated reader sends it.
const STANDING_SQL: &str = include_str!("../../queries/commission/standing.sql");

/// The participant table, written by direct statements: no port writes a non-owner row yet.
struct PgParticipantRows(PgPool);

#[async_trait]
impl ParticipantRows for PgParticipantRows {
    async fn add(&self, commission: &CommissionId, user: &UserId) {
        adapter_pg::queries::commission::add_participant(
            &self.0,
            uuid::Uuid::from(*commission),
            user.as_ref(),
            Utc::now(),
        )
        .await
        .expect("add the participant row");
    }

    async fn remove(&self, commission: &CommissionId, user: &UserId) {
        // The owner's row is guarded by a trigger; replica mode skips it for this one delete.
        let mut tx = self.0.begin().await.expect("begin");
        sqlx::query("SET LOCAL session_replication_role = replica")
            .execute(&mut *tx)
            .await
            .expect("skip the owner floor's trigger");
        let removed = sqlx::query(
            "DELETE FROM commission_participant WHERE commission_id = $1 AND user_id = $2",
        )
        .bind(uuid::Uuid::from(*commission))
        .bind(user.as_ref())
        .execute(&mut *tx)
        .await
        .expect("remove the participant row");
        assert_eq!(removed.rows_affected(), 1, "one participant row is removed");
        tx.commit().await.expect("commit the removal");
    }
}

/// A fresh database's unit factory, commission store, participant rows and commission seeder.
async fn fresh() -> (
    Arc<dyn Database>,
    Arc<dyn CommissionStore>,
    Arc<dyn ParticipantRows>,
    Arc<dyn CommissionSeeder>,
    impl Sized,
) {
    let (pool, kept) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());
    let rows = PgParticipantRows(pool);
    (
        Arc::new(database),
        Arc::new(store),
        Arc::new(rows),
        Arc::new(MintingSeeder),
        kept,
    )
}

test_support::commission_standing_contract_tests!(fresh);

/// Recognizes `did` as a User in a unit of its own.
async fn provision(database: &dyn Database, did: &str) -> UserId {
    let mut unit = database.begin().await.expect("begin");
    let user = unit
        .users()
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision the user");
    unit.commit().await.expect("commit the user");
    user.id
}

/// Seeds a Private commission owned by `owner` in a unit of its own.
async fn committed_commission(database: &dyn Database, owner: &UserId) -> CommissionId {
    seeder::seed_commission(
        database,
        &MintingSeeder,
        owner,
        Level::Private,
        "A made-up commission",
    )
    .await
}

/// Whether a second connection's `FOR NO KEY UPDATE NOWAIT` on the commission row is refused
/// because another transaction holds the row.
async fn the_row_is_held(pool: &PgPool, commission: &CommissionId) -> bool {
    let locked = sqlx::query("SELECT 1 FROM commission WHERE id = $1 FOR NO KEY UPDATE NOWAIT")
        .bind(uuid::Uuid::from(*commission))
        .execute(pool)
        .await;
    let Err(error) = locked else {
        return false;
    };
    let code = error
        .as_database_error()
        .and_then(|database_error| database_error.code())
        .map(|code| code.into_owned());
    assert_eq!(
        code.as_deref(),
        Some("55P03"),
        "lock_not_available: {error}"
    );
    true
}

// A node level outside the vocabulary (stored with `node_level_known` dropped in this test's
// own database) reads as Private, on both reads.
#[tokio::test]
async fn an_unreadable_node_level_reads_private_on_both_reads() {
    let (pool, _kept) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());
    let owner = provision(&database, "did:plc:unreadableowner").await;
    let commission = committed_commission(&database, &owner).await;
    allow_any_node_level(&pool).await;
    store_node_level(&pool, commission, "public-ish").await;
    let stored_level: String = sqlx::query_scalar(
        "SELECT n.level FROM node n JOIN commission c ON c.node_id = n.id WHERE c.id = $1",
    )
    .bind(uuid::Uuid::from(commission))
    .fetch_one(&pool)
    .await
    .expect("read the stored token");
    assert_eq!(stored_level, "public-ish", "the token is stored as written");

    let expected = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: false,
    };
    let pooled = store
        .standing(&commission, &owner)
        .await
        .expect("the pool's standing read");
    assert_eq!(pooled, Some(expected), "standing");
    let mut unit = database.begin().await.expect("begin");
    let locked = unit
        .commissions()
        .standing_for_update(&commission, &owner)
        .await
        .expect("the locked standing read");
    let locked_standing = locked.map(|locked| locked.standing);
    assert_eq!(locked_standing, Some(expected), "standing_for_update");
}

// A stranger's standing on a real commission and a standing on a made-up id each run exactly
// the one statement `commission/standing`, outside any transaction, and never the whole row;
// the made-up id's row says not found.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_run_the_same_one_statement() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let runtime = &pg.runtime;
    let owner = provision(&*runtime.database, "did:plc:statementowner").await;
    let stranger = provision(&*runtime.database, "did:plc:statementstranger").await;
    let commission = committed_commission(&*runtime.database, &owner).await;
    let made_up = CommissionId::from(uuid::Uuid::now_v7());

    let cases = [("a stranger", commission), ("a made-up id", made_up)];
    for (label, target) in cases {
        let mark = statements.mark();
        runtime
            .commissions
            .standing(&target, &stranger)
            .await
            .expect("the pool's standing read");
        let ran = statements.since(mark);
        let names: Vec<String> = ran.iter().map(ToString::to_string).collect();
        assert_eq!(names, ["commission/standing"], "{label}: {names:?}");
    }

    let row = adapter_pg::queries::commission::standing(
        &runtime.pool,
        uuid::Uuid::from(made_up),
        stranger.as_ref(),
    )
    .await
    .expect("the standing statement");
    let [row] = <[_; 1]>::try_from(row).expect("one row for a made-up id");
    assert_eq!(row.found, Some(false), "a made-up id is not found");
    let answered = runtime
        .commissions
        .standing(&made_up, &stranger)
        .await
        .expect("the pool's standing read");
    assert_eq!(answered, None, "a made-up id answers not found");
}

// The shipped standing statement probes the node index for a made-up id as for a stranger's
// real commission: neither plan reads `node` conditionally, so the two answer in the same steps.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_both_probe_the_node_index() {
    let (pool, _kept) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let owner = provision(&database, "did:plc:planowner").await;
    let stranger = provision(&database, "did:plc:planstranger").await;
    let commission = committed_commission(&database, &owner).await;
    let made_up = CommissionId::from(uuid::Uuid::now_v7());
    let explain = format!("EXPLAIN (ANALYZE, COSTS OFF, TIMING OFF) {STANDING_SQL}");

    let cases = [("a made-up id", made_up), ("a stranger", commission)];
    for (label, target) in cases {
        let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(explain.clone()))
            .bind(uuid::Uuid::from(target))
            .bind(stranger.as_ref())
            .fetch_all(&pool)
            .await
            .expect("explain the standing statement");
        assert_the_node_probe_runs(label, &plan);
    }
}

// A unit that ran `standing_for_update` holds the commission row: a second connection can't
// lock it until the unit ends.
#[tokio::test]
async fn standing_for_update_holds_the_commission_row() {
    let (pool, _kept) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let owner = provision(&database, "did:plc:lockowner").await;
    let commission = committed_commission(&database, &owner).await;
    assert!(
        !the_row_is_held(&pool, &commission).await,
        "nothing holds the row before the read"
    );

    let mut unit = database.begin().await.expect("begin");
    unit.commissions()
        .standing_for_update(&commission, &owner)
        .await
        .expect("the locked standing read");
    assert!(
        the_row_is_held(&pool, &commission).await,
        "the unit holds the row after its locked read"
    );
    unit.rollback().await.expect("end the unit");
    assert!(
        !the_row_is_held(&pool, &commission).await,
        "the row is free once the unit ends"
    );
}

// The system's lock reads no viewer: it answers the commission's node and level, holds the
// commission row until its unit ends, and answers nothing for a made-up id.
#[tokio::test]
async fn lock_for_system_answers_the_node_and_holds_the_commission_row() {
    let (pool, _kept) = test_support::pg::fresh_pool().await;
    let database = PgDatabase::new(pool.clone());
    let owner = provision(&database, "did:plc:systemlockowner").await;
    let commission = committed_commission(&database, &owner).await;
    let made_up = CommissionId::from(uuid::Uuid::now_v7());

    let mut unit = database.begin().await.expect("begin");
    let locked = unit
        .commissions()
        .lock_for_system(&commission)
        .await
        .expect("the system's locked read")
        .expect("a commission with a node");
    let standing = unit
        .commissions()
        .standing_for_update(&commission, &owner)
        .await
        .expect("the locked standing read")
        .expect("the owner's standing");
    let absent = unit
        .commissions()
        .lock_for_system(&made_up)
        .await
        .expect("the system's locked read");
    assert!(
        the_row_is_held(&pool, &commission).await,
        "the unit holds the row after the system's lock"
    );
    unit.rollback().await.expect("end the unit");

    assert_eq!(
        locked.node, standing.node,
        "the node the commission points at"
    );
    assert_eq!(locked.level, standing.standing.level);
    assert_eq!(absent, None);
    assert!(
        !the_row_is_held(&pool, &commission).await,
        "the row is free once the unit ends"
    );
}

// A standing change committed while `standing_for_update` waited for the commission row is
// what the locked read answers: a participant evicted under the row's lock reads as no role.
#[tokio::test]
async fn standing_for_update_sees_a_change_committed_while_it_waited() {
    let (pool, _kept) = test_support::pg::fresh_pool().await;
    let database = Arc::new(PgDatabase::new(pool.clone()));
    let owner = provision(&*database, "did:plc:raceowner").await;
    let member = provision(&*database, "did:plc:racemember").await;
    let commission = committed_commission(&*database, &owner).await;
    PgParticipantRows(pool.clone())
        .add(&commission, &member)
        .await;

    let mut evict = pool.begin().await.expect("begin the eviction");
    sqlx::query("SELECT 1 FROM commission WHERE id = $1 FOR NO KEY UPDATE")
        .bind(uuid::Uuid::from(commission))
        .execute(&mut *evict)
        .await
        .expect("the eviction locks the commission row first");
    sqlx::query("DELETE FROM commission_participant WHERE commission_id = $1 AND user_id = $2")
        .bind(uuid::Uuid::from(commission))
        .bind(member.as_ref())
        .execute(&mut *evict)
        .await
        .expect("evict the member");

    let reader = Arc::clone(&database);
    let read_member = member.clone();
    let waiting = tokio::spawn(async move {
        let mut unit = reader.begin().await.expect("begin");
        let standing = unit
            .commissions()
            .standing_for_update(&commission, &read_member)
            .await
            .expect("the locked standing read");
        unit.rollback().await.expect("end the unit");
        standing.map(|locked| locked.standing)
    });
    wait_for_a_lock_wait(&pool).await;
    evict.commit().await.expect("commit the eviction");

    let standing = waiting.await.expect("the locked read finishes");
    let expected = CommissionStanding {
        role: None,
        level: Level::Private,
        archived: false,
    };
    assert_eq!(standing, Some(expected), "the eviction is seen");
}

/// Waits, for at most ten seconds, until some connection to this database waits on a lock.
async fn wait_for_a_lock_wait(pool: &PgPool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
             WHERE datname = current_database() AND wait_event_type = 'Lock')",
        )
        .fetch_one(pool)
        .await
        .expect("read pg_stat_activity");
        if waiting {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no connection waited on the commission row within ten seconds"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
