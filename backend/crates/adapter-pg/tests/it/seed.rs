//! The PostgreSQL harness's commission seeding: the port's create, under a capability this
//! test code mints, for the shared suites and this binary's own tests. A seeded commission
//! is what the den's write gate writes: its node, its six Private entries, and the
//! commission bound to that node.

use adapter_pg::{PgDatabase, PgPool};
use async_trait::async_trait;
use domain::elements::{
    commission::{CommissionBirth, CommissionId, NewCommission},
    level::Level,
    user::UserId,
};
use domain::ports::{New, UnitOfWork, WriteCapability};
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
