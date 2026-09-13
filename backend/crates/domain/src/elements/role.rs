//! [`Role`] — a member's rank inside an account, and the rule for who may grant
//! what. Four ranks, Owner highest; only Owner and Admin may change roles, and
//! the grant rule itself lives in [`Role::can_grant`].

use std::str::FromStr;

/// A member's rank inside one account. The derived [`Ord`] runs Owner < Admin <
/// Manager < Member, so a lower position means higher authority and
/// [`can_grant`](Role::can_grant) leans on it — **keep the variants in rank
/// order**.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The account's founder and highest authority; never has a parent.
    Owner,
    /// May change roles below Admin; cannot mint a peer Admin or an Owner.
    Admin,
    /// A member with elevated standing but no authority to change roles.
    Manager,
    /// The base membership rank; grants nothing.
    Member,
}

impl Role {
    /// The stored discriminant [`FromStr`] parses back.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Manager => "manager",
            Self::Member => "member",
        }
    }

    pub fn is_administrative(&self) -> bool {
        matches!(self, Role::Owner | Role::Admin)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stored role discriminant outside the four known roles — a schema-drift
/// signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl std::fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown role {:?}", self.0)
    }
}

impl std::error::Error for UnknownRole {}

impl FromStr for Role {
    type Err = UnknownRole;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let role = match s.to_lowercase().as_str() {
            "owner" => Role::Owner,
            "admin" => Role::Admin,
            "manager" => Role::Manager,
            "member" => Role::Member,
            _ => return Err(UnknownRole(s.into())),
        };

        Ok(role)
    }
}

impl Role {
    /// Whether a member holding `self` may grant `target` to another member —
    /// the one authority seam. Only an Owner or Admin grants at all, and only a
    /// role strictly below their own: granting Owner is transfer, its own seam.
    pub fn can_grant(&self, target: &Role) -> bool {
        // Two parts: only Owner/Admin grant at all (a Manager outranks a Member
        // but grants nothing), and the target must sit strictly below the actor
        // — with the derived Ord, "below" is the greater value.
        matches!(self, Role::Owner | Role::Admin) && target > self
    }
}

/// A member's alias for their own role on an account — a free-form label, never
/// a second authority axis: it lives on the membership, not on [`Role`], so it
/// can never influence the derived [`Ord`] or
/// [`can_grant`](Role::can_grant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleAlias(String);

/// A role alias that failed validation — empty after trimming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidRoleAlias;

impl std::fmt::Display for InvalidRoleAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a role alias cannot be empty")
    }
}

impl std::error::Error for InvalidRoleAlias {}

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
