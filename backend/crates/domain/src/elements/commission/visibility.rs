use super::UnknownVisibility;

/// Who may see a commission — the outermost gate, applied before the
/// composition's own [`effective_visibility`]. A fresh commission is
/// [`Private`](Visibility::Private); widening is an explicit later act.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    /// Nobody outside the participants sees it at all, not even its existence.
    Private,
    /// Outsiders see only a status-only card.
    Listed,
    /// Outsiders see whatever sits under Description-visible surfaces.
    Public,
}

impl Visibility {
    /// The stable, lowercase token written to `commission.visibility`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Listed => "listed",
            Self::Public => "public",
        }
    }
}

impl TryFrom<&str> for Visibility {
    type Error = UnknownVisibility;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "private" => Self::Private,
            "listed" => Self::Listed,
            "public" => Self::Public,
            _ => return Err(UnknownVisibility),
        })
    }
}
