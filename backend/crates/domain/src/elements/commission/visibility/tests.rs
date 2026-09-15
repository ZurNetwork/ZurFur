use strum::VariantArray;

use super::*;

// The visibility tokens round-trip and never collide.
#[test]
fn visibility_tokens_round_trip_and_never_collide() {
    let tokens: Vec<&'static str> = Visibility::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["private", "listed", "public"]);

    for visibility in Visibility::VARIANTS {
        let token = <&'static str>::from(visibility);
        assert_eq!(visibility.to_string(), token);
        let parsed: Visibility = token.parse().expect("a valid token must parse");
        assert_eq!(parsed, visibility.clone());
    }
}

// A token outside the vocabulary is refused.
#[test]
fn unknown_visibility_tokens_do_not_parse() {
    assert!(
        "Private".parse::<Visibility>().is_err(),
        "tokens are lowercase"
    );
    assert!("".parse::<Visibility>().is_err());
    assert!("total".parse::<Visibility>().is_err());
}
