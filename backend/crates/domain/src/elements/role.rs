//! [`Role`] — a member's rank inside an account, and the rule for who may grant
//! what.
//!
//! DESIGN/Roles is the source of truth. There are four ranks (Owner is highest,
//! Member lowest); only Owner and Admin may change roles; granting a role is how
//! a user joins, revoking it is how they leave. The grant rule lives in
//! [`Role::can_grant`] — the reusable authority seam ZMVP-15/16 are built on.

/// The optional alias a member's role carries — the `Option<String>` slot on each
/// [`Role`] variant, named so the four variants read uniformly. `None` on the floor
/// (see [`Role`]). A `Some` is a free-form label for the rank (e.g. an Owner aliased
/// "Studio Head"), not a second authority axis: [`can_grant`](Role::can_grant) ranks
/// by variant, not by alias. Distinct from a member's *parent* — that is the
/// inviting member, stored in a separate `account_members.parent` column, never here.
pub type RoleAlias = Option<String>;

/// A member's rank inside one account.
///
/// Ordered Owner < Admin < Manager < Member by the derived [`Ord`], so a *lower*
/// numeric position means *higher* authority — [`can_grant`](Role::can_grant)
/// leans on this, so keep the variants in rank order, top to bottom.
///
/// Each variant carries an optional [`RoleAlias`] — a free-form label for the rank,
/// not the member's parent (that is a separate `account_members.parent` column). On
/// the floor it is always `None`. Be aware the derived [`Ord`] also weighs that
/// `Option<String>`; that is why aliased-role compares are deferred dressing (see
/// the note on [`can_grant`](Role::can_grant)).
///
/// References: [`UnknownRole`], [`crate::elements::user_account::UserAccount`],
/// [`crate::ports::AccountWrites::grant_role`], DESIGN/Roles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The account's founder/highest authority; never has a parent (DESIGN/Roles).
    Owner(RoleAlias),
    /// May change roles below Admin; cannot mint a peer Admin or an Owner.
    Admin(RoleAlias),
    /// A member with elevated standing but no authority to change roles.
    Manager(RoleAlias),
    /// The base membership rank; grants nothing.
    Member(RoleAlias),
}

/// A stored role discriminant that isn't one of the four known roles.
///
/// The error returned by [`Role::try_from`] when a persisted string doesn't map
/// to a [`Role`] — a schema/data drift signal, not a user input error. Carries
/// the offending value for diagnostics.
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
    /// back into a `Role`. The [`RoleAlias`] is `None`: the discriminant alone can't
    /// carry it, and it is always NULL on the floor. When aliases land, reconstruct
    /// the alias alongside, e.g. via a `TryFrom<(String, Option<String>)>`.
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
        // keep it Owner→Member, top to bottom. (When aliases start getting populated,
        // switch to a discriminant-only compare — Ord also weighs that `Option<String>`
        // alias, which would otherwise reopen the peer-Admin gap for aliased roles.)
        matches!(self, Role::Owner(_) | Role::Admin(_)) && target > self
    }

    /// The lowercase discriminant (`owner` | `admin` | `manager` | `member`),
    /// the value persisted by the store and the inverse of [`Role::try_from`].
    /// Drops the alias — only the rank is encoded here.
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
