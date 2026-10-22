//! `MemBackend::seed_bound_commission` writes what the den's write gate writes, through a
//! unit whose node rules and commit checks run over it: a seed that breaks them is refused
//! and keeps nothing.

use adapter_mem::{CommissionSeed, MemBackend};
use domain::elements::{
    commission::CommissionTitle, did::Did, level::Level, node::NodeType, user::UserId,
};

/// Recognizes `did` as a User on `backend`.
async fn user(backend: &MemBackend, did: &str) -> UserId {
    backend
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision the user")
        .id
}

/// A Public commission titled "A seeded piece", owned by `owner`.
fn public_seed(owner: &UserId) -> CommissionSeed {
    let title = "A seeded piece"
        .parse::<CommissionTitle>()
        .expect("a valid title");
    CommissionSeed {
        level: Level::Public,
        ..CommissionSeed::private(title, owner.clone())
    }
}

/// A seeded commission stands at its own node, with its six entries, and reads its title
/// and level from it; seeding its id again is refused and adds no node.
#[tokio::test]
async fn a_seeded_commission_obeys_the_binding() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:seeding-owner").await;
    let seed = public_seed(&owner);

    let node_id = backend
        .seed_bound_commission(&seed)
        .await
        .expect("seed the commission");

    let segment = seed
        .id
        .to_string()
        .parse()
        .expect("a commission id is a segment");
    let node = backend
        .node_store()
        .at_top(&NodeType::Commission, &segment)
        .await
        .expect("a read")
        .expect("the commission's node");
    assert_eq!(node.id, node_id, "the answered node is its own");
    assert_eq!(node.level, Level::Public, "the node holds the seed's level");
    let entries = backend
        .node_store()
        .children(&node.id)
        .await
        .expect("a read");
    assert_eq!(entries.len(), 6, "its six entries");
    assert!(
        entries.iter().all(|entry| entry.level == Level::Private),
        "every entry is Private"
    );
    let found = backend
        .commission_store()
        .find_summary(&seed.id)
        .await
        .expect("find")
        .expect("the commission is stored");
    assert_eq!(found.title.as_str(), "A seeded piece", "the node's name");
    assert_eq!(found.visibility, Level::Public, "the node's level");

    let again = backend.seed_bound_commission(&seed).await;
    assert!(again.is_err(), "a second seed of the id is refused");
    assert_eq!(
        backend.all_nodes().len(),
        7,
        "the refused seed adds no node"
    );
}

/// A seed the create refuses, for an owner no User holds, keeps none of its nodes.
#[tokio::test]
async fn a_refused_seed_keeps_nothing() {
    let backend = MemBackend::new();
    let stranger = UserId::from(Did::from("did:plc:nobody-seeded".to_owned()));
    let seed = public_seed(&stranger);

    let seeded = backend.seed_bound_commission(&seed).await;

    assert!(seeded.is_err(), "an unknown owner is refused");
    assert!(backend.all_nodes().is_empty(), "no node is kept");
    let found = backend.find_commission(seed.id).await.expect("find");
    assert!(found.is_none(), "no commission is kept");
}
