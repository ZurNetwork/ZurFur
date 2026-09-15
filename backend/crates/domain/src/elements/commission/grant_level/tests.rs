use std::collections::BTreeSet;

use super::*;

// The closed grant-level vocabulary. `GrantLevel` exposes no public `ALL`,
// so the round-trip test names its own test-only list.
const ALL_GRANT_LEVELS: &[GrantLevel] = &[
    GrantLevel::Presentation,
    GrantLevel::Description,
    GrantLevel::Total,
];

// The grant-level tokens round-trip and never collide.
#[test]
fn grant_level_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for level in ALL_GRANT_LEVELS {
        let token = level.to_string();
        assert!(seen.insert(token.clone()), "duplicate token {token:?}");
        let parsed: GrantLevel = token.parse().expect("a valid token must parse");
        assert_eq!(
            parsed, *level,
            "token {token:?} must parse back to its level"
        );
    }
    assert_eq!(ALL_GRANT_LEVELS.len(), 3, "exactly three modes exist");
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
