//! The platform-wide maturity rating primitive: the atproto self-label
//! vocabulary adopted as Zurfur's own — Safe / Suggestive / Nudity / Adult
//! ([`MaturityRating`]) plus an orthogonal Graphic flag, together one
//! [`Maturity`] value.
//!
//! There is no mapping layer: the network self-label a rating emits is derived
//! from it, never chosen separately.

use std::str::FromStr;

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

/// Why a token was rejected as a [`MaturityRating`].
#[derive(Debug, PartialEq, Eq)]
pub enum MaturityRatingError {
    /// The token is outside the four-value vocabulary.
    UnknownRating,
}

impl std::fmt::Display for MaturityRatingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownRating => {
                write!(f, "expected one of: safe, suggestive, nudity, adult")
            }
        }
    }
}

impl std::error::Error for MaturityRatingError {}

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

/// A work's complete maturity posture: the [`MaturityRating`] plus the
/// orthogonal Graphic flag, which rides alongside any rating rather than being a
/// fifth tier. One value by design — a work is either unrated or carries both,
/// so a graphic flag without a rating is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maturity {
    /// The four-tier axis value.
    pub rating: MaturityRating,
    /// Whether the work is graphic (gore/violence) — orthogonal to the axis.
    pub graphic: bool,
}

impl Maturity {
    /// The self-label the Graphic flag emits.
    pub const GRAPHIC_LABEL: &'static str = "graphic-media";

    /// Every atproto self-label this posture emits at publish time. Empty
    /// means Safe and not graphic. Publish paths wrap these into
    /// [`SelfLabels`](crate::elements::public_record::SelfLabels); commissions
    /// never do.
    pub fn self_labels(&self) -> Vec<&'static str> {
        let mut labels: Vec<&'static str> = self.rating.self_label().into_iter().collect();
        if self.graphic {
            labels.push(Self::GRAPHIC_LABEL);
        }
        labels
    }
}

#[cfg(test)]
mod tests {
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

    // The axis table: Safe emits none, Suggestive -> sexual, Nudity -> nudity,
    // Adult -> porn; Graphic rides alongside any rating.
    #[test]
    fn self_labels_follow_the_dd_axis_table() {
        assert_eq!(MaturityRating::Safe.self_label(), None);
        assert_eq!(MaturityRating::Suggestive.self_label(), Some("sexual"));
        assert_eq!(MaturityRating::Nudity.self_label(), Some("nudity"));
        assert_eq!(MaturityRating::Adult.self_label(), Some("porn"));

        let safe = Maturity {
            rating: MaturityRating::Safe,
            graphic: false,
        };
        assert!(safe.self_labels().is_empty(), "Safe + not graphic = empty");

        let graphic_safe = Maturity {
            rating: MaturityRating::Safe,
            graphic: true,
        };
        assert_eq!(
            graphic_safe.self_labels(),
            vec!["graphic-media"],
            "Graphic is orthogonal — it rides even on Safe",
        );

        let graphic_adult = Maturity {
            rating: MaturityRating::Adult,
            graphic: true,
        };
        assert_eq!(graphic_adult.self_labels(), vec!["porn", "graphic-media"]);
    }
}
