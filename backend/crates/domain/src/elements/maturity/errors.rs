/// Why a token was rejected as a [`MaturityRating`](super::MaturityRating).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MaturityRatingError {
    /// The token is outside the four-value vocabulary.
    #[error("expected one of: safe, suggestive, nudity, adult")]
    UnknownRating,
}
