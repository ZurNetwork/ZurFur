use std::collections::BTreeSet;

use strum::VariantArray;

use super::*;

// The storage tokens round-trip and never collide.
#[test]
fn kind_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for kind in ChangelogEntryKind::VARIANTS {
        let token = <&'static str>::from(*kind);
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(
            token.parse::<ChangelogEntryKind>().ok(),
            Some(*kind),
            "token {token:?} must parse back to its kind",
        );
    }
}

// A token outside the vocabulary is refused, not guessed at.
#[test]
fn unknown_tokens_do_not_parse() {
    assert_eq!("placement_changed".parse::<ChangelogEntryKind>().ok(), None);
    assert_eq!("".parse::<ChangelogEntryKind>().ok(), None);
    assert_eq!("CREATED".parse::<ChangelogEntryKind>().ok(), None);
}
