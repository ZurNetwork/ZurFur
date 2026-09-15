use std::collections::BTreeSet;

use strum::VariantArray;

use super::*;

// The storage tokens round-trip and never collide.
#[test]
fn rating_tokens_round_trip_and_never_collide() {
    let tokens: Vec<&'static str> = MaturityRating::VARIANTS
        .iter()
        .map(|rating| <&'static str>::from(*rating))
        .collect();
    assert_eq!(tokens, ["safe", "suggestive", "nudity", "adult"]);

    let mut seen = BTreeSet::new();
    for rating in MaturityRating::VARIANTS {
        let token = <&'static str>::from(*rating);
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(rating.to_string(), token);
        assert_eq!(token.parse::<MaturityRating>(), Ok(*rating));
        assert_eq!(
            MaturityRating::try_from(token),
            Ok(*rating),
            "token {token:?} must parse back to its rating",
        );
    }
}

// Only the enum's own tokens parse — case variants and the derived label
// values are all refused.
#[test]
fn out_of_vocabulary_tokens_do_not_parse() {
    for bad in [
        "questionable",
        "explicit",
        "Safe",
        "ADULT",
        "sexual",
        "porn",
        "graphic-media",
        "",
    ] {
        assert_eq!(
            MaturityRating::try_from(bad),
            Err(MaturityRatingError::UnknownRating),
            "{bad:?} must be outside the vocabulary",
        );
        assert_eq!(
            bad.parse::<MaturityRating>(),
            Err(MaturityRatingError::UnknownRating)
        );
    }
}
