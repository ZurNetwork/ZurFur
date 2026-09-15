//! Commission positioning: the account-facing rails that replaced the deleted
//! managing-account concept. Users own commissions; neither rail confers any
//! in-commission authority.
//!
//! Placement is not here — a commission's placement IS its card on an account's
//! board, so it lives in `domain::elements::workflow`. What remains is
//! [`GrantLevel`], the level of a commission-side key to see.

use std::str::FromStr;

/// The level a view grant confers — one of the three raw root modes, explicitly
/// chosen at grant time with no default. A grant is issued to a **User**, never
/// an account, and a user's effective view is the max of their own standing and
/// their own key; membership confers no view. Hard-deleted on revoke.
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
mod tests;
