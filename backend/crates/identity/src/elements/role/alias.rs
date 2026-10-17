use std::str::FromStr;

use super::errors::InvalidRoleAlias;

/// A member's alias for their own role on an account — a free-form label, never
/// a second authority axis: it lives on the membership, not on [`Role`], so it
/// can never influence the derived [`Ord`] or
/// [`can_grant`](Role::can_grant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleAlias(String);

impl RoleAlias {
    /// Trims `value` and rejects it if nothing is left. Free-form otherwise —
    /// no charset or length rule.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidRoleAlias> {
        let trimmed = value.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(InvalidRoleAlias);
        }
        Ok(Self(trimmed))
    }

    /// The trimmed alias text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoleAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RoleAlias {
    type Err = InvalidRoleAlias;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests;
