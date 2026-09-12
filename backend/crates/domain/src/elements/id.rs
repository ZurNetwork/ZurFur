//! [`IdError`] — the shared parse failure every domain id newtype's `FromStr`
//! returns, and the private UUID helper the UUID-backed ones delegate to.

use std::str::FromStr;

/// Why a string failed to parse as a domain id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The input isn't a valid UUID — from the UUID-backed ids (commissions,
    /// elements, invitations, actor-identity rows).
    NotAUuid,
    /// The input isn't valid for the id's own representation — from the
    /// DID-backed actor ids ([`UserId`], [`AccountId`], [`CharacterId`]).
    ///
    /// [`UserId`]: crate::elements::user::UserId
    /// [`AccountId`]: crate::elements::account::AccountId
    /// [`CharacterId`]: crate::elements::character::CharacterId
    ParsingError,
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAUuid => write!(f, "value is not a UUID"),
            Self::ParsingError => write!(f, "value is not a valid identifier"),
        }
    }
}

impl std::error::Error for IdError {}

/// Parses `s` as a UUID, mapping any failure to [`IdError::NotAUuid`] — the one
/// helper every UUID-backed id newtype's `FromStr` delegates to.
pub(crate) fn parse_uuid(s: &str) -> Result<uuid::Uuid, IdError> {
    uuid::Uuid::from_str(s).map_err(|_| IdError::NotAUuid)
}
