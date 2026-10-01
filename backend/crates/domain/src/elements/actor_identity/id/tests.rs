use chrono::Utc;

use super::*;
use crate::elements::actor_identity::{ActorIdentity, ActorKind};

/// Every mint is a distinct row-to-be.
#[test]
fn mint_yields_distinct_ids() {
    assert_ne!(
        ActorIdentity::mint(ActorKind::User, Utc::now()).id,
        ActorIdentity::mint(ActorKind::User, Utc::now()).id
    );
}

/// The id round-trips through its stored UUID.
#[test]
fn id_rebuilds_from_stored_uuid() {
    let minted = ActorIdentity::mint(ActorKind::Account, Utc::now());
    assert_eq!(
        ActorIdentityId::from(uuid::Uuid::from(minted.id)),
        minted.id
    );
}
