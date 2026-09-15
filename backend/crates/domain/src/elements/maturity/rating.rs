use super::MaturityRatingError;

/// The four-tier maturity axis. Chosen per individual work and enforced
/// server-side: a value reaches storage only through this enum, so an
/// out-of-vocabulary rating is unrepresentable past the boundary.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = MaturityRatingError,
    parse_err_fn = unknown_token
)]
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

/// The typed error for a token outside the vocabulary.
fn unknown_token(_token: &str) -> MaturityRatingError {
    MaturityRatingError::UnknownRating
}

impl MaturityRating {
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

#[cfg(test)]
mod tests;
