//! Commission positioning: the account-facing rails that replaced the deleted
//! managing-account concept. Users own commissions; neither rail confers any
//! in-commission authority. (DD 29130754)
//!
//! Placement is not here — a commission's placement IS its card on an account's
//! board, so it lives in `domain::elements::workflow`. What remains is
//! [`GrantLevel`], the level of a commission-side key to see.

use std::str::FromStr;

/// The level a view grant confers — one of the three raw root modes, explicitly
/// chosen at grant time with no default. A grant is issued to a **User**, never
/// an account, and a user's effective view is the max of their own standing and
/// their own key; membership confers no view. Hard-deleted on revoke.
/// (DD 29130754, as amended 2026-09-04)
///
/// Not the [`Visibility`](super::Visibility) aliases: a grant speaks the
/// underlying mode directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantLevel {
    /// The narrowest key: the Presentation-mode projection.
    Presentation,
    /// A middle key: whatever is composed under Description-visible surfaces.
    Description,
    /// The widest key: Participant-equivalent view.
    Total,
}

impl std::fmt::Display for GrantLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Presentation => write!(f, "presentation"),
            Self::Description => write!(f, "description"),
            Self::Total => write!(f, "total"),
        }
    }
}
#[derive(Debug)]
pub struct GrantLevelError;
impl std::fmt::Display for GrantLevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Grant level parsing error")
    }
}
impl std::error::Error for GrantLevelError {}
impl FromStr for GrantLevel {
    type Err = GrantLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "presentation" => Self::Presentation,
            "description" => Self::Description,
            "total" => Self::Total,
            _ => Err(GrantLevelError)?,
        })
    }
}

#[cfg(test)]
mod tests {
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
}
