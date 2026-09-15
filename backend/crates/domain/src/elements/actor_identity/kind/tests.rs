use strum::VariantArray;

use super::*;

/// Every kind's spelling round-trips through its pinned token; an unknown or
/// wrongly-cased one is a loud, typed error.
#[test]
fn kind_spelling_round_trips() {
    let tokens: Vec<&'static str> = ActorKind::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["user", "account", "character"]);

    for kind in ActorKind::VARIANTS {
        let token = <&'static str>::from(kind);
        assert_eq!(kind.to_string(), token);
        assert_eq!(token.parse::<ActorKind>(), Ok(*kind));
        assert_eq!(ActorKind::try_from(token), Ok(*kind));
    }

    assert_eq!(
        ActorKind::try_from("golem"),
        Err(UnknownActorKind("golem".into()))
    );
    assert_eq!(ActorKind::try_from(""), Err(UnknownActorKind("".into())));
    assert_eq!(
        ActorKind::try_from("User"),
        Err(UnknownActorKind("User".into()))
    );
}
