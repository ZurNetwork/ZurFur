//! Shared helpers for adapter-mem's integration tests: a commission created the way the den's
//! write gate creates one, under a capability this test code mints.
// Each test binary uses its own share of the helpers.
#![allow(dead_code)]

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{CommissionBirth, CommissionId, NewCommission},
    level::Level,
    node::{NewNode, NodeId, NodeSegment, NodeType},
    text::StoredText,
    user::UserId,
};
use domain::ports::{New, UnitOfWork, WriteCapability};
use strum::VariantArray;
use test_support::seeder::CommissionSeeder;

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

/// The commission node the gate writes for `id` at `level`, named `name`.
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

/// The six Private entries under the commission node `parent`, each its word.
pub fn entry_nodes(parent: NodeId) -> Vec<NewNode> {
    NodeType::VARIANTS
        .iter()
        .filter_map(|entry_type| {
            let word: &'static str = entry_type.into();
            let word = word.strip_prefix("commission.")?;
            let entry = NewNode {
                id: NodeId::mint(),
                parent: Some(parent),
                segment: NodeSegment::try_from(word).expect("an entry's word is a segment"),
                node_type: *entry_type,
                level: Level::Private,
                name: StoredText::from(word.to_owned()),
            };
            Some(entry)
        })
        .collect()
}

/// Writes `nodes` on `unit`, in order.
pub async fn write_nodes(unit: &mut dyn UnitOfWork, nodes: &[NewNode]) -> anyhow::Result<()> {
    let mut repo = unit.nodes();
    for node in nodes {
        repo.create(node).await?;
    }
    Ok(())
}

/// A starting state chosen with no maturity and no deadline.
pub fn unrated() -> NewCommission {
    NewCommission {
        maturity: None,
        deadline: None,
    }
}

/// Writes on `unit` a new commission's node at `level` named `name` with its six entries,
/// then creates the commission there owned by `owner`, created now. Answers its id and node.
pub async fn create_bound(
    unit: &mut dyn UnitOfWork,
    owner: &UserId,
    level: Level,
    name: &str,
) -> anyhow::Result<(CommissionId, NodeId)> {
    let id = CommissionId::mint();
    let node = commission_node(id, level, name);
    let mut nodes = vec![node.clone()];
    nodes.extend(entry_nodes(node.id));
    write_nodes(unit, &nodes).await?;
    let birth = CommissionBirth {
        node: node.id,
        owner_id: owner.clone(),
        created_at: Utc::now(),
    };
    create(unit, id, &birth, &unrated()).await?;
    Ok((id, node.id))
}
