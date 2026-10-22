//! How visible a thing is, [`Level`], ordered lowest first, with its stored tokens.

/// How visible a thing is to anyone without a relationship to it, lowest first:
/// Private < Listed < Public.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownLevel,
    parse_err_fn = unknown_token
)]
pub enum Level {
    /// Absent: not even its existence shows.
    Private,
    /// A card: it shows that it exists, but it can't be opened.
    Listed,
    /// Open to anyone who can reach it.
    Public,
}

/// Why a token failed to resolve to a [`Level`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: private, listed, public")]
pub struct UnknownLevel;

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> UnknownLevel {
    UnknownLevel
}

#[cfg(test)]
mod tests;
