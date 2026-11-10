use strum::VariantArray;

use super::*;

// The stored tokens, in declaration order.
#[test]
fn the_tokens_are_these() {
    let tokens: Vec<&'static str> = Level::VARIANTS
        .iter()
        .map(|level| <&'static str>::from(*level))
        .collect();

    assert_eq!(tokens, ["private", "listed", "public"]);
}

// Every token round-trips through Display and both parses.
#[test]
fn every_token_round_trips() {
    for level in Level::VARIANTS {
        let token = <&'static str>::from(*level);

        assert_eq!(level.to_string(), token);
        assert_eq!(token.parse::<Level>(), Ok(*level));
        assert_eq!(Level::try_from(token), Ok(*level));
    }
}

// Only the lowercase tokens parse; anything else is the typed refusal.
#[test]
fn a_token_outside_the_vocabulary_is_refused() {
    for bad in ["Private", "PUBLIC", "", "total", "hidden", "public "] {
        assert_eq!(bad.parse::<Level>(), Err(UnknownLevel), "{bad:?}");
        assert_eq!(Level::try_from(bad), Err(UnknownLevel), "{bad:?}");
    }
}

// The order is Private, then Listed, then Public.
#[test]
fn private_is_below_listed_is_below_public() {
    let mut levels = [Level::Public, Level::Private, Level::Listed];
    levels.sort();

    assert_eq!(levels, [Level::Private, Level::Listed, Level::Public]);
    assert!(Level::Private < Level::Listed);
    assert!(Level::Listed < Level::Public);
}
