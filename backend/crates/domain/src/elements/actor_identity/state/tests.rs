use chrono::Utc;
use strum::VariantArray;

use super::*;
use crate::elements::actor_identity::{ActorIdentity, ActorKind};

/// Every state's spelling round-trips through its pinned token, a row is born
/// Active, and an unknown or wrongly-cased token is a loud, typed error.
#[test]
fn state_spelling_round_trips_and_mint_is_active() {
    let tokens: Vec<&'static str> = ActorState::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["active", "pulled", "tombstoned"]);

    for state in ActorState::VARIANTS {
        let token = <&'static str>::from(state);
        assert_eq!(state.to_string(), token);
        assert_eq!(token.parse::<ActorState>(), Ok(*state));
        assert_eq!(ActorState::try_from(token), Ok(*state));
    }

    assert_eq!(
        ActorState::try_from("deleted"),
        Err(UnknownActorState("deleted".into()))
    );
    assert_eq!(ActorState::try_from(""), Err(UnknownActorState("".into())));
    assert_eq!(
        ActorState::try_from("Active"),
        Err(UnknownActorState("Active".into()))
    );

    assert_eq!(
        ActorIdentity::mint(ActorKind::User, Utc::now()).state,
        ActorState::Active
    );
}
