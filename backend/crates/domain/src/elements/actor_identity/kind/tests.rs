use super::*;

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
