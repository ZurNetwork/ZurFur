/// Why a token was rejected as a [`MaturityRating`](super::MaturityRating).
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
