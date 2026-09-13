use chrono::Utc;

use super::*;

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
    assert_eq!(ActorIdentityId::new(*minted.id), minted.id);
}

/// Every kind's spelling parses back; an unknown one is a loud error.
#[test]
fn kind_spelling_round_trips() {
    for kind in [ActorKind::User, ActorKind::Account, ActorKind::Character] {
        assert_eq!(ActorKind::try_from(kind.as_str()), Ok(kind));
    }
    assert_eq!(
        ActorKind::try_from("golem"),
        Err(UnknownActorKind("golem".to_string()))
    );
}

/// Every state's spelling parses back, and rows are born Active.
#[test]
fn state_spelling_round_trips_and_mint_is_active() {
    for state in [
        ActorState::Active,
        ActorState::Pulled,
        ActorState::Tombstoned,
    ] {
        assert_eq!(ActorState::try_from(state.as_str()), Ok(state));
    }
    assert_eq!(
        ActorState::try_from("deleted"),
        Err(UnknownActorState("deleted".to_string()))
    );
    assert_eq!(
        ActorIdentity::mint(ActorKind::User, Utc::now()).state,
        ActorState::Active
    );
}
