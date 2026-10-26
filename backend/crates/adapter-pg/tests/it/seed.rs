//! The PostgreSQL harness's commission seeding: the port's create, under a capability this
//! test code mints, for the shared suites and this binary's own tests. A seeded commission
//! is what the den's write gate writes: its node, its six Private entries, and the
//! commission bound to that node.

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{
        AsParticipant, Commission, CommissionBirth, CommissionDeadline, CommissionId,
        LifecycleStep, NewCommission,
    },
    level::Level,
    user::UserId,
};
use domain::ports::{CommissionStore, Database, New, UnitOfWork, WriteCapability};
use test_support::seeder::{self, CommissionSeeder};

/// Creates commission `id` on `unit` through the port, under a capability minted here.
pub async fn create(
    unit: &mut dyn UnitOfWork,
    id: CommissionId,
    birth: &CommissionBirth,
    commission: &NewCommission,
) -> anyhow::Result<()> {
    let capability = WriteCapability::gate_issued(New::from(id));
    unit.commissions()
        .create(&capability, birth, commission)
        .await
}

/// A Participant's capability on `id`, minted here as the den's write gate issues one.
pub fn participant_on(id: CommissionId) -> WriteCapability<CommissionId, AsParticipant> {
    WriteCapability::gate_issued(id)
}

/// The shared suites' [`CommissionSeeder`]: the port's create, under a capability minted here.
pub struct MintingSeeder;

#[async_trait]
impl CommissionSeeder for MintingSeeder {
    async fn create(
        &self,
        unit: &mut dyn UnitOfWork,
        id: CommissionId,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()> {
        create(unit, id, birth, commission).await
    }
}

/// Seeds, committed, a Private commission titled `title` and owned by `owner`.
pub async fn seed_titled(pool: &PgPool, owner: &UserId, title: &str) -> CommissionId {
    let database = PgDatabase::new(pool.clone());
    seeder::seed_commission(&database, &MintingSeeder, owner, Level::Private, title).await
}

/// [`seed_titled`], answered as the store holds it.
pub async fn seed_stored(pool: &PgPool, owner: &UserId, title: &str) -> Commission {
    let id = seed_titled(pool, owner, title).await;
    stored(pool, id).await
}

/// Seeds, committed, a Private commission titled `title` and owned by `owner`, with the
/// chosen `deadline`, and (when given) staged into `step` by a test-side update; staged into
/// Disputed, it is disputed from Active since now. Answers it as the store holds it.
pub async fn seed_chosen(
    pool: &PgPool,
    owner: &UserId,
    title: &str,
    deadline: Option<chrono::DateTime<Utc>>,
    step: Option<LifecycleStep>,
) -> Commission {
    let id = CommissionId::mint();
    let node = seeder::commission_node(id, Level::Private, title);
    let birth = CommissionBirth {
        node: node.id,
        owner_id: owner.clone(),
        created_at: Utc::now(),
    };
    let chosen = NewCommission {
        maturity: None,
        deadline: deadline.map(CommissionDeadline::from),
    };
    let database = PgDatabase::new(pool.clone());
    let mut unit = database.begin().await.expect("begin");
    {
        let mut nodes = unit.nodes();
        nodes
            .create(&node)
            .await
            .expect("store the commission node");
        for entry in seeder::entry_nodes(node.id) {
            nodes.create(&entry).await.expect("store an entry");
        }
    }
    create(&mut *unit, id, &birth, &chosen)
        .await
        .expect("create the commission");
    unit.commit().await.expect("commit the commission");
    if let Some(step) = step {
        sqlx::query(
            "UPDATE commission SET lifecycle = $2,
                 disputed_from = CASE WHEN $2 = 'disputed' THEN 'active' END,
                 disputed_at = CASE WHEN $2 = 'disputed' THEN now() END
             WHERE id = $1",
        )
        .bind(uuid::Uuid::from(id))
        .bind(<&'static str>::from(&step))
        .execute(pool)
        .await
        .expect("stage the lifecycle step");
    }
    stored(pool, id).await
}

/// Commission `id` as the store holds it.
async fn stored(pool: &PgPool, id: CommissionId) -> Commission {
    PgCommissionStore::new(pool.clone())
        .find(&id)
        .await
        .expect("find the seeded commission")
        .expect("the seeded commission is stored")
}
