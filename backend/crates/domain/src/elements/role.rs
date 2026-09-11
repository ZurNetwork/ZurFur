//! [`Role`] — a member's rank inside an account, and the rule for who may grant
//! what. Four ranks, Owner highest; only Owner and Admin may change roles, and
//! the grant rule itself lives in [`Role::can_grant`]. (DESIGN 2162692)

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
    /// (DESIGN 2162692)
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
mod tests {
    use super::*;

    // The full actor -> target matrix for `can_grant` — where the rule is
    // pinned, since the e2e suite can only sign in the Owner.
    #[test]
    fn can_grant_matrix_matches_the_design() {
        let roles = || [Role::Owner, Role::Admin, Role::Manager, Role::Member];
        for actor in roles() {
            for target in roles() {
                let expected = match (&actor, &target) {
                    (Role::Owner, Role::Owner) => false,
                    (Role::Owner, _) => true,
                    (Role::Admin, Role::Owner | Role::Admin) => false,
                    (Role::Admin, _) => true,
                    _ => false,
                };
                assert_eq!(
                    actor.can_grant(&target),
                    expected,
                    "{actor:?} granting {target:?}"
                );
            }
        }
    }

    // The sharpest edges: no one grants Owner through this seam, an Admin
    // cannot mint a peer Admin, and the lower roles grant nothing.
    #[test]
    fn no_owner_no_peer_admin_no_lower_role_grants() {
        for actor in [Role::Owner, Role::Admin, Role::Manager, Role::Member] {
            assert!(!actor.can_grant(&Role::Owner), "{actor:?} → Owner");
        }
        assert!(
            !Role::Admin.can_grant(&Role::Admin),
            "an Admin cannot grant Admin"
        );
        for actor in [Role::Manager, Role::Member] {
            assert!(!actor.can_grant(&Role::Member), "{actor:?} grants");
        }
    }

    #[test]
    fn every_role_round_trips_through_as_str_and_parse() {
        for role in [Role::Owner, Role::Admin, Role::Manager, Role::Member] {
            let parsed: Role = role.as_str().parse().expect("as_str is always parseable");
            assert_eq!(parsed, role);
        }
    }

    #[test]
    fn role_alias_trims_surrounding_whitespace() {
        let alias = RoleAlias::new("  Studio Head  ").expect("non-empty after trimming");
        assert_eq!(alias.as_str(), "Studio Head");
    }

    #[test]
    fn role_alias_rejects_empty_and_whitespace_only() {
        assert_eq!(RoleAlias::new(""), Err(InvalidRoleAlias));
        assert_eq!(RoleAlias::new("   "), Err(InvalidRoleAlias));
    }

    #[test]
    fn role_alias_round_trips_through_display_and_from_str() {
        let alias = RoleAlias::new("Studio Head").expect("non-empty");
        let round_tripped: RoleAlias = alias.to_string().parse().expect("re-parses");
        assert_eq!(alias, round_tripped);
    }
}
