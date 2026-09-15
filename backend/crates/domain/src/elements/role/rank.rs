use std::str::FromStr;

use super::errors::UnknownRole;

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

#[cfg(test)]
mod tests;
