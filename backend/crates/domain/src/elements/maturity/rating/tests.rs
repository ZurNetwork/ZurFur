use std::collections::BTreeSet;

use super::*;

// The storage tokens round-trip and never collide.
#[test]
fn rating_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for rating in MaturityRating::ALL {
        let token = rating.as_str();
        assert!(seen.insert(token), "duplicate token {token:?}");
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
        assert!(
            MaturityRating::try_from(bad).is_err(),
            "{bad:?} must be outside the vocabulary",
        );
    }
}
