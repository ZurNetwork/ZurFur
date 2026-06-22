#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Owner(Option<String>),
    Admin(Option<String>),
    Manager(Option<String>),
    Member(Option<String>),
}

/// A stored role discriminant that isn't one of the four known roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl std::fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown role {:?}", self.0)
    }
}

impl std::error::Error for UnknownRole {}

impl TryFrom<String> for Role {
    type Error = UnknownRole;

    /// Parse a stored role discriminant (`owner` | `admin` | `manager` | `member`)
    /// back into a `Role`. The parent slot is `None`: the discriminant alone can't
    /// carry it — the parent is a separate column, and is always NULL on the floor
    /// (only Owner exists). When the role tree lands, reconstruct the parent
    /// alongside, e.g. via a `TryFrom<(String, Option<String>)>`.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "owner" => Ok(Role::Owner(None)),
            "admin" => Ok(Role::Admin(None)),
            "manager" => Ok(Role::Manager(None)),
            "member" => Ok(Role::Member(None)),
            _ => Err(UnknownRole(value)),
        }
    }
}

impl Role {
    /// Whether a member holding `self` (the actor) may grant `target` to another
    /// member — the reusable role-check seam ZMVP-15 is built to be born at.
    ///
    /// The rule is DESIGN/Roles, source of truth:
    /// - "Only `Owner` and `Admin` may change roles." → the actor must be one of them.
    /// - The granted role sits *strictly below* the actor's own rank: an Owner grants
    ///   Admin and below, an Admin grants Manager and below — never a peer Admin (that
    ///   would let Admins mint Admins) and never Owner (granting Owner is *transfer*,
    ///   its own seam — "an Owner never has a parent, even when transferred").
    /// - "`Manager` and `Member` cannot change anyone's role."
    ///
    /// Truth table (actor → grantable targets):
    ///   Owner   → Admin, Manager, Member   (never Owner)
    ///   Admin   → Manager, Member          (never Admin, never Owner)
    ///   Manager → nothing
    ///   Member  → nothing
    ///
    /// Not yet enforced (deferred dressing, DESIGN/Roles): the parent/child hierarchy
    /// tree and demotion limited to one's own subtree.
    pub fn can_grant(&self, target: &Role) -> bool {
        // Two parts, because the rule isn't pure rank. (1) Only Owner and Admin may
        // grant at all: a Manager outranks a Member but still grants nothing, so
        // `target > self` alone would wrongly let a Manager seat a Member. (2) The
        // granted role must sit strictly below the actor's — with the derived Ord
        // (Owner < Admin < Manager < Member) "below" is the greater value, hence
        // `target > self`. Authority therefore rides on the variant order above:
        // keep it Owner→Member, top to bottom. (When the role tree populates the
        // parent slot, switch to a discriminant-only compare — Ord also weighs that
        // `Option<String>`, which would reopen the peer-Admin gap for parented roles.)
        matches!(self, Role::Owner(_) | Role::Admin(_)) && target > self
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner(_) => "owner",
            Role::Admin(_) => "admin",
            Role::Manager(_) => "manager",
            Role::Member(_) => "member",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The full actor → target matrix for `can_grant`, straight off DESIGN/Roles.
    // The e2e suite can only sign in the Owner, so the Admin/Manager/Member actor
    // rows live here — this is where the rule is pinned.
    #[test]
    fn can_grant_matrix_matches_the_design() {
        let roles = || {
            [
                Role::Owner(None),
                Role::Admin(None),
                Role::Manager(None),
                Role::Member(None),
            ]
        };
        for actor in roles() {
            for target in roles() {
                // An actor grants only roles strictly below its own rank, and only
                // Owner and Admin may grant at all (Manager and Member grant nothing).
                let expected = match (&actor, &target) {
                    (Role::Owner(_), Role::Owner(_)) => false,
                    (Role::Owner(_), _) => true,
                    (Role::Admin(_), Role::Owner(_) | Role::Admin(_)) => false,
                    (Role::Admin(_), _) => true,
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

    // The sharpest edges, stated outright: no one grants Owner through this seam
    // (transfer is its own path), an Admin cannot mint a peer Admin, and the lower
    // roles grant nothing at all.
    #[test]
    fn no_owner_no_peer_admin_no_lower_role_grants() {
        for actor in [
            Role::Owner(None),
            Role::Admin(None),
            Role::Manager(None),
            Role::Member(None),
        ] {
            assert!(!actor.can_grant(&Role::Owner(None)), "{actor:?} → Owner");
        }
        assert!(
            !Role::Admin(None).can_grant(&Role::Admin(None)),
            "an Admin cannot grant Admin"
        );
        for actor in [Role::Manager(None), Role::Member(None)] {
            assert!(!actor.can_grant(&Role::Member(None)), "{actor:?} grants");
        }
    }
}
