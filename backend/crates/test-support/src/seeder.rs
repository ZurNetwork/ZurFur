//! Seeds commissions the way the den's write gate writes a create, for the shared suites:
//! a top-level commission node whose segment is the commission's id, its six Private
//! entries, and the commission bound to that node. This crate never mints a write
//! capability: each adapter's harness implements [`CommissionSeeder`] in its own test code.

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{CommissionBirth, CommissionId, NewCommission, SlotBirth},
    did::Did,
    level::Level,
    node::{NewNode, NodeId, NodeSegment, NodeType},
    text::StoredText,
    user::UserId,
};
use domain::ports::{Database, UnitOfWork};

use crate::node_contract::{ENTRY_TYPES, entry_node};

/// The made-up owner of a commission a case binds without one of its own.
const MADE_UP_OWNER: &str = "did:plc:seededowner";

/// Creates a commission through the commission port, under a capability the adapter's own
/// test code mints.
#[async_trait]
pub trait CommissionSeeder: Send + Sync {
    /// Creates commission `id` on `unit` at `birth`'s node, with what `commission` chose.
    async fn create(
        &self,
        unit: &mut dyn UnitOfWork,
        id: CommissionId,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()>;
}

/// Declares Slots through the commission port, under an Owner's capability the adapter's own
/// test code mints, on commissions its harness reads committed.
#[async_trait]
pub trait SlotSeeder: Send + Sync {
    /// Declares on `unit` the Slot `birth` names on the committed commission `id`, its element
    /// in that commission's first tab and surface, bound to `birth`'s node.
    async fn declare_slot(
        &self,
        unit: &mut dyn UnitOfWork,
        id: CommissionId,
        birth: &SlotBirth,
    ) -> anyhow::Result<()>;
}

/// Recognizes the made-up owner on `unit`.
pub async fn made_up_owner(unit: &mut dyn UnitOfWork) -> UserId {
    let user = unit
        .users()
        .provision(&Did::from(MADE_UP_OWNER.to_owned()))
        .await
        .expect("provision the made-up owner");
    user.id
}

/// Binds a commission owned by `owner` to `node`, a commission node `unit` holds: the
/// commission's id is the node's segment. Answers that id.
pub async fn bind(
    unit: &mut dyn UnitOfWork,
    seeder: &dyn CommissionSeeder,
    node: &NewNode,
    owner: &UserId,
) -> CommissionId {
    let id = node
        .segment
        .as_ref()
        .parse::<CommissionId>()
        .expect("a bound commission node's segment is a commission id");
    let birth = CommissionBirth {
        node: node.id,
        owner_id: owner.clone(),
        created_at: Utc::now(),
    };
    let chosen = NewCommission {
        maturity: None,
        deadline: None,
    };
    seeder
        .create(unit, id, &birth, &chosen)
        .await
        .expect("create the commission bound to its node");
    id
}

/// The commission node the gate writes for a new commission `id` at `level`, named `name`.
pub fn commission_node(id: CommissionId, level: Level, name: &str) -> NewNode {
    let segment = NodeSegment::try_from(id.to_string()).expect("a commission id is a segment");
    NewNode {
        id: NodeId::mint(),
        parent: None,
        segment,
        node_type: NodeType::Commission,
        level,
        name: StoredText::from(name.to_owned()),
    }
}

/// The six Private entries the gate writes under the commission node `parent`.
pub fn entry_nodes(parent: NodeId) -> Vec<NewNode> {
    ENTRY_TYPES
        .iter()
        .map(|entry_type| entry_node(parent, *entry_type))
        .collect()
}

/// Writes on `unit` a new commission named `name`, owned by `owner` at `level`, as the gate
/// writes a create: its commission node, its six Private entries, and the commission bound
/// to that node.
pub async fn write_commission(
    unit: &mut dyn UnitOfWork,
    seeder: &dyn CommissionSeeder,
    owner: &UserId,
    level: Level,
    name: &str,
) -> CommissionId {
    let node = commission_node(CommissionId::mint(), level, name);
    {
        let mut nodes = unit.nodes();
        nodes
            .create(&node)
            .await
            .expect("store the commission node");
        for entry in entry_nodes(node.id) {
            nodes.create(&entry).await.expect("store an entry");
        }
    }
    bind(unit, seeder, &node, owner).await
}

/// [`write_commission`] in a unit of its own, committed.
pub async fn seed_commission(
    database: &dyn Database,
    seeder: &dyn CommissionSeeder,
    owner: &UserId,
    level: Level,
    name: &str,
) -> CommissionId {
    let mut unit = database.begin().await.expect("begin");
    let id = write_commission(&mut *unit, seeder, owner, level, name).await;
    unit.commit().await.expect("commit the commission");
    id
}
