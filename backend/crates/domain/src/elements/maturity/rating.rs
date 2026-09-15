use std::str::FromStr;

use super::MaturityRatingError;

/// The four-tier maturity axis. Chosen per individual work and enforced
/// server-side: a value reaches storage only through this enum, so an
/// out-of-vocabulary rating is unrepresentable past the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaturityRating {
    /// No maturity concern; emits no network label.
    Safe,
    /// Sexually suggestive (→ the `sexual` self-label).
    Suggestive,
    /// Non-sexual nudity (→ the `nudity` self-label).
    Nudity,
    /// Adult content (→ the `porn` self-label).
    Adult,
}

impl MaturityRating {
    /// Every rating, in axis order — the closed vocabulary.
    pub const ALL: &[MaturityRating] = &[Self::Safe, Self::Suggestive, Self::Nudity, Self::Adult];

    /// The stable, lowercase token written to `commission.maturity` — the
    /// Zurfur rating name, not the network label. Persisted, so renaming one is
    /// a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Suggestive => "suggestive",
            Self::Nudity => "nudity",
            Self::Adult => "adult",
        }
    }

    /// The atproto self-label this rating emits at publish time. Safe emits no
    /// label — an empty label set means safe. Derived, never chosen separately.
    pub fn self_label(&self) -> Option<&'static str> {
        match self {
            Self::Safe => None,
            Self::Suggestive => Some("sexual"),
            Self::Nudity => Some("nudity"),
            Self::Adult => Some("porn"),
        }
    }
}

impl TryFrom<&str> for MaturityRating {
    type Error = MaturityRatingError;

    /// Resolve a stored or submitted token back to its rating. A token outside
    /// the four values is an error, never a silent default.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "safe" => Self::Safe,
            "suggestive" => Self::Suggestive,
            "nudity" => Self::Nudity,
            "adult" => Self::Adult,
            _ => return Err(MaturityRatingError::UnknownRating),
        })
    }
}

impl std::fmt::Display for MaturityRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Safe => write!(f, "safe"),
            Self::Suggestive => write!(f, "suggestive"),
            Self::Nudity => write!(f, "nudity"),
            Self::Adult => write!(f, "adult"),
        }
    }
}

impl FromStr for MaturityRating {
    type Err = MaturityRatingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "safe" => Self::Safe,
            "suggestive" => Self::Suggestive,
            "nudity" => Self::Nudity,
            "adult" => Self::Adult,
            _ => return Err(MaturityRatingError::UnknownRating),
        })
    }
}

#[cfg(test)]
mod tests;
