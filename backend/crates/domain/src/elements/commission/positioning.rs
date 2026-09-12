//! Commission **positioning** (ZMVP-70; Ownership Separation DD `29130754`):
//! the two account-facing rails that replace the deleted managing-account
//! concept. Users own commissions; accounts own positioning, and neither rail
//! confers any in-commission authority (DD Decision 8 — the environmental rule).
//!
//! Two shapes live here, kept apart exactly as the DD keeps them (Decision 6 —
//! "they never share a table"):
//! - [`Placement`] — an **account-side** row in the commission's append-only
//!   placement log: the commission was placed in an account's position. The log
//!   is never rewritten; the current placement is its latest row, the origin its
//!   first. A denormalized current-placement pointer is kept in step with the
//!   latest row (the pg `commission_current_placement` cache).
//! - [`GrantLevel`] — the level of a **commission-side** *key to see*, issued to
//!   an account at an explicitly chosen level (Decision 3). A key only lifts an
//!   account's members to at least its level, never demotes (Decision 4); it
//!   **hard-deletes on revoke**, effective on the next server-side serialization
//!   (Decision 5 — no session to invalidate). The grant row is a *pure key* (just
//!   the level per (commission, account)) — who issued it and when live only in
//!   the changelog (Decision 5: "grant history lives only in the changelog").
//!
//! Neither rail is a [`Fact`](super::Fact): both are commission-owned
//! bookkeeping that cascades away with the commission (the tables are registered
//! in `COMMISSION_NON_FACT_TABLES`).

use std::str::FromStr;

use crate::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, commission::CommissionId, user::UserId},
};

/// The level a **view grant** confers — one of the three **raw root modes**
/// (Ownership Separation DD `29130754` Decision 3). A grant names a mode floor
/// the account's members are lifted to; the level is **explicitly chosen at
/// grant time**, with no default and no implicit escalation.
///
/// Deliberately **not** the [`Visibility`](super::Visibility) aliases
/// (Private/Listed/Public): those are the commission-level *root* vocabulary,
/// while a grant speaks the underlying mode directly — `Presentation`,
/// `Description`, or `Total`. A `Total` key is the serious one: it extends
/// Participant-equivalent *view* (brief, client identity, price, file entries)
/// to the account's entire present-and-future membership until revoked (DD
/// "Accepted tradeoff").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantLevel {
    /// The narrowest key: the account sees the commission's Presentation-mode
    /// projection (existence, title, maturity — the status-card tier).
    Presentation,
    /// A middle key: the account sees whatever is composed under
    /// Description-visible surfaces.
    Description,
    /// The widest key: Participant-equivalent view of the whole tree.
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

/// One **placement** of a commission into an account's position — a row in the
/// append-only placement log (ZMVP-70; Ownership Separation DD `29130754`
/// Decision 1/6). The commission never knows it is placed; positioning is
/// account-side view state. Placement confers **no** in-commission authority
/// (Decision 8).
///
/// The log is never rewritten: re-placement appends a new row. `seq` is the
/// store-assigned ordering key (a pg `bigserial`), so the **current** placement
/// is the row with the greatest `seq` and the **origin** is the least. This same
/// shape is returned for the denormalized current-placement pointer, which is
/// kept equal to the latest row after every (re)placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The store-assigned ordering key (pg `bigserial`) — monotonic, so the
    /// greatest `seq` is the current placement and the least is the origin.
    pub seq: i64,
    /// The commission being positioned.
    pub commission_id: CommissionId,
    /// The account into whose position the commission was placed.
    pub account_id: AccountId,
    /// The User who performed the placement (the commission owner in v1).
    pub placed_by: UserId,
    /// When the placement happened.
    pub placed_at: DateTimeUtc,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    // The closed grant-level vocabulary, in declaration order. `GrantLevel`
    // deliberately dropped its public `ALL` with the move to `Display`/`FromStr`
    // (only the wire tokens are public API); the round-trip test below still
    // needs every variant, so it names its own local, test-only list.
    const ALL_GRANT_LEVELS: &[GrantLevel] = &[
        GrantLevel::Presentation,
        GrantLevel::Description,
        GrantLevel::Total,
    ];

    // The grant-level tokens are a closed, collision-free vocabulary that
    // round-trips — the same contract the changelog kinds hold.
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
        assert_eq!(
            ALL_GRANT_LEVELS.len(),
            3,
            "exactly three modes exist (DD D3)"
        );
    }

    // A token outside the vocabulary is refused, not guessed at — and the grant
    // vocabulary is the raw modes, never the Visibility aliases.
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
