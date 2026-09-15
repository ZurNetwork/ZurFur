use chrono::Utc;

use super::*;
use crate::elements::actor_identity::{ActorIdentity, ActorKind};

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
