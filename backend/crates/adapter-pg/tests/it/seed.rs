//! The PostgreSQL harness's commission seeding: the port's create, under a capability this
//! test code mints, for the shared suites and this binary's own tests. A seeded commission
//! is what the den's write gate writes: its node, its six Private entries, and the
//! commission bound to that node.

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{
        AsOwner, AsParticipant, Commission, CommissionBirth, CommissionDeadline, CommissionId,
        ElementId, LifecycleStep, NewCommission, NewSlot, SKELETON, SlotBirth, SlotDeclaration,
        SurfaceAddress, SurfaceName,
    },
    level::Level,
    node::{NewNode, NodeId, NodeSegment, NodeType},
    text::StoredText,
    user::UserId,
};
use domain::ports::{CommissionStore, Database, New, UnitOfWork, WriteCapability};
use test_support::seeder::{self, CommissionSeeder, SlotSeeder};

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

/// The Owner's capability on `id`, minted here as the den's write gate issues one.
pub fn owner_on(id: CommissionId) -> WriteCapability<CommissionId, AsOwner> {
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

/// The committed commission `id`'s one skeleton address: its first tab and surface.
pub async fn only_address(pool: &PgPool, id: CommissionId) -> anyhow::Result<SurfaceAddress> {
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(&id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("a committed commission composes"))?;
    let tab = composition
        .tabs
        .first()
        .ok_or_else(|| anyhow::anyhow!("a committed commission has its tab"))?
        .id;
    let surface = SKELETON[0].surfaces[0].parse::<SurfaceName>()?;
    Ok(SurfaceAddress::new(tab, surface))
}

/// The node of commission `id`'s `slots` entry, read on `unit`.
pub async fn slots_entry_in(unit: &mut dyn UnitOfWork, id: CommissionId) -> anyhow::Result<NodeId> {
    let top = NodeSegment::try_from(id.to_string())?;
    let commission_node = unit
        .nodes()
        .at_top(&NodeType::Commission, &top)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the commission has its node"))?;
    let word = NodeSegment::try_from("slots")?;
    let entry = unit
        .nodes()
        .child(&commission_node.id, &word)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the commission has its slots entry"))?;
    Ok(entry.id)
}

/// A Slot's node named `title` for `element` under the `slots` entry `parent`.
pub fn slot_node(parent: NodeId, element: ElementId, title: &str) -> anyhow::Result<NewNode> {
    let node = NewNode {
        id: NodeId::mint(),
        parent: Some(parent),
        segment: NodeSegment::try_from(element.to_string())?,
        node_type: NodeType::Slot,
        level: Level::Private,
        name: StoredText::from(title.to_owned()),
    };
    Ok(node)
}

/// Declares on `unit` a Slot titled `title` at `address` with `notes` on commission `id`, by
/// `declarer`, as the den's write gate does: its node under the commission's `slots` entry,
/// then the port's declare under an Owner's capability minted here. Answers its element id.
pub async fn declare_in(
    unit: &mut dyn UnitOfWork,
    id: CommissionId,
    declarer: &UserId,
    address: &SurfaceAddress,
    title: &str,
    notes: Option<&str>,
) -> anyhow::Result<ElementId> {
    let slot = NewSlot {
        address: address.clone(),
        notes: notes.map(str::to_owned),
    };
    let declaration = SlotDeclaration::minted(slot);
    let parent = slots_entry_in(unit, id).await?;
    let node = slot_node(parent, declaration.element, title)?;
    unit.nodes().create(&node).await?;
    let birth = SlotBirth {
        element: declaration.element,
        node: node.id,
        created_by: declarer.clone(),
        created_at: Utc::now(),
    };
    unit.commissions()
        .declare_slot(&owner_on(id), &birth, &declaration.slot)
        .await?;
    Ok(declaration.element)
}

/// [`declare_in`] in a unit of its own, committed.
pub async fn declare_committed(
    pool: &PgPool,
    id: CommissionId,
    declarer: &UserId,
    address: &SurfaceAddress,
    title: &str,
    notes: Option<&str>,
) -> ElementId {
    let database = PgDatabase::new(pool.clone());
    let mut unit = database.begin().await.expect("begin");
    let element = declare_in(&mut *unit, id, declarer, address, title, notes)
        .await
        .expect("the Slot is declared");
    unit.commit().await.expect("commit the Slot");
    element
}

/// The shared suites' [`SlotSeeder`]: the port's declare, under an Owner's capability minted
/// here, at the committed commission's first tab and surface, read from its pool.
pub struct MintingSlotSeeder(pub PgPool);

#[async_trait]
impl SlotSeeder for MintingSlotSeeder {
    async fn declare_slot(
        &self,
        unit: &mut dyn UnitOfWork,
        id: CommissionId,
        birth: &SlotBirth,
    ) -> anyhow::Result<()> {
        let address = only_address(&self.0, id).await?;
        let slot = NewSlot {
            address,
            notes: None,
        };
        unit.commissions()
            .declare_slot(&owner_on(id), birth, &slot)
            .await
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
