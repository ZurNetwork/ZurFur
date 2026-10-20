use strum::VariantArray;

use super::*;

// The grant-level tokens round-trip and never collide.
#[test]
fn grant_level_tokens_round_trip_and_never_collide() {
    let tokens: Vec<&'static str> = GrantLevel::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["presentation", "description", "total"]);

    for level in GrantLevel::VARIANTS {
        let token = <&'static str>::from(level);
        assert_eq!(level.to_string(), token);
        let parsed: GrantLevel = token.parse().expect("a valid token must parse");
        assert_eq!(
            parsed, *level,
            "token {token:?} must parse back to its level"
        );
    }
}

// A token outside the vocabulary is refused; grants speak raw modes.
#[test]
fn unknown_and_alias_tokens_do_not_parse() {
    assert!("".parse::<GrantLevel>().is_err());
    assert!(
        "Total".parse::<GrantLevel>().is_err(),
        "tokens are lowercase"
    );
    assert!(
        "private".parse::<GrantLevel>().is_err(),
        "a grant speaks raw modes, never the Private/Listed/Public aliases",
    );
    assert!("listed".parse::<GrantLevel>().is_err());
}
