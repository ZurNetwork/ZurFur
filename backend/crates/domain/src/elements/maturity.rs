//! The platform-wide **maturity rating** primitive (ZMVP-31; Maturity
//! Vocabulary DD `29982722`).
//!
//! Zurfur adopts the atproto self-label vocabulary as its own maturity system:
//! the axis is **Safe / Suggestive / Nudity / Adult** ([`MaturityRating`]),
//! plus an orthogonal **Graphic** flag — gore is not a sexual-maturity
//! question (DD Decision 2). Together they form one [`Maturity`] value. The DD
//! scopes this vocabulary to *everywhere* maturity lives — commissions,
//! Products, Gallery Posts, any future rated surface (Decision 3) — which is
//! why the primitive lives here as its own element rather than inside any one
//! of them.
//!
//! There is **no mapping layer**: the network self-label each rating emits is
//! *derived* from it — never chosen separately (DD Decisions 1 and 4;
//! [`MaturityRating::self_label`]). For Class B surfaces like commissions the
//! rating never leaves the Index at all; the label derivation exists for the
//! publish paths (Gallery Posts and their
//! [`SelfLabels`](crate::elements::public_record::SelfLabels) wire shape) to
//! consume when their epics land.

/// The four-tier maturity axis — the atproto self-label vocabulary adopted as
/// Zurfur's own (Maturity Vocabulary DD `29982722`, Decision 1; supersedes the
/// pre-DD Safe/Questionable/Explicit placeholder).
///
/// The rating is chosen per individual work and enforced **server-side**: a
/// value reaches storage only through this enum (its `TryFrom<&str>` at the
/// boundary), so out-of-vocabulary ratings are unrepresentable past it
/// (ZMVP-31 "values from the enum only").
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
    /// Every rating, in axis order — the closed vocabulary. Lets tests prove
    /// the token mapping round-trips and stays collision-free, and gives UI
    /// layers the dropdown order for free.
    pub const ALL: &[MaturityRating] = &[Self::Safe, Self::Suggestive, Self::Nudity, Self::Adult];

    /// The stable, lowercase wire/storage token for this rating — the value
    /// the pg adapter writes to the `commission.maturity` column and the API
    /// accepts. Stable across releases (it is persisted), so renaming a token
    /// is a migration, not a free edit. These are the **Zurfur rating names**;
    /// the network label each one emits is [`self_label`](Self::self_label).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Suggestive => "suggestive",
            Self::Nudity => "nudity",
            Self::Adult => "adult",
        }
    }

    /// The atproto self-label this rating emits when content carrying it is
    /// published — the DD's Decision-1 axis table, verbatim: Safe emits *no*
    /// label (the protocol norm: an empty label set means safe), the rest map
    /// onto the global self-label values. Derived, never chosen separately
    /// (Decision 4).
    pub fn self_label(&self) -> Option<&'static str> {
        match self {
            Self::Safe => None,
            Self::Suggestive => Some("sexual"),
            Self::Nudity => Some("nudity"),
            Self::Adult => Some("porn"),
        }
    }
}

/// A token that isn't one of the four [`MaturityRating`] values.
///
/// The error half of [`MaturityRating`]'s `TryFrom<&str>` conversion — a token
/// outside the closed vocabulary. At the API boundary that becomes a `422`
/// (server-side enforcement, ZMVP-31 AC); on a read path it means row tampering
/// or a missed migration and surfaces as an error, never a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownMaturityRating;

impl std::fmt::Display for UnknownMaturityRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("token is not one of: safe, suggestive, nudity, adult")
    }
}

impl std::error::Error for UnknownMaturityRating {}

impl TryFrom<&str> for MaturityRating {
    type Error = UnknownMaturityRating;

    /// Resolve a stored/submitted token back to its rating — an explicit `match`
    /// on the closed vocabulary, the mirror of [`as_str`](Self::as_str) and the
    /// same shape as [`LifecycleStep`] / [`Visibility`]. A token outside the four
    /// values is [`UnknownMaturityRating`], never a silent default.
    ///
    /// [`LifecycleStep`]: crate::elements::commission::LifecycleStep
    /// [`Visibility`]: crate::elements::commission::Visibility
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "safe" => Self::Safe,
            "suggestive" => Self::Suggestive,
            "nudity" => Self::Nudity,
            "adult" => Self::Adult,
            _ => return Err(UnknownMaturityRating),
        })
    }
}

/// A work's complete maturity posture: the four-tier [`MaturityRating`] plus
/// the orthogonal **Graphic** flag (Maturity Vocabulary DD `29982722`,
/// Decision 2 — "gore is not a sexual-maturity question", so it rides
/// alongside *any* rating rather than being a fifth tier).
///
/// The two halves are one value by design: a work is either unrated
/// (`Option<Maturity>` = `None` — legal only while nothing outside its
/// participants can see it) or carries both. A graphic flag without a rating
/// is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maturity {
    /// The four-tier axis value.
    pub rating: MaturityRating,
    /// Whether the work is graphic (gore/violence) — orthogonal to the axis.
    pub graphic: bool,
}

impl Maturity {
    /// The self-label the Graphic flag emits (DD Decision 2: Graphic →
    /// `graphic-media`).
    pub const GRAPHIC_LABEL: &'static str = "graphic-media";

    /// Every atproto self-label this posture emits at publish time — the
    /// rating's [`self_label`](MaturityRating::self_label) plus
    /// [`GRAPHIC_LABEL`](Self::GRAPHIC_LABEL) when graphic. Empty = Safe and
    /// not graphic (the protocol's empty-set norm). Publish paths wrap these
    /// into the [`SelfLabels`](crate::elements::public_record::SelfLabels)
    /// wire shape; commissions (Class B) never do.
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

    // The storage tokens are a closed, collision-free vocabulary that
    // round-trips — the same contract every persisted enum here carries.
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

    // Server-side enforcement: only the enum's own tokens parse. The
    // superseded pre-DD vocabulary (questionable/explicit), case variants,
    // and the *label* values are all refused — a rating is chosen as a
    // rating, never smuggled in as its derived label.
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

    // The DD's Decision-1 axis table, verbatim: Safe emits none, Suggestive →
    // sexual, Nudity → nudity, Adult → porn; Graphic rides alongside any
    // rating as graphic-media (Decision 2).
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
