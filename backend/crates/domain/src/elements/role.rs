//! [`Role`] — a member's rank inside an account, and the rule for who may grant
//! what.
//!
//! DESIGN/Roles is the source of truth. There are four ranks (Owner is highest,
//! Member lowest); only Owner and Admin may change roles; granting a role is how
//! a user joins, revoking it is how they leave. The grant rule lives in
//! [`Role::can_grant`] — the reusable authority seam ZMVP-15/16 are built on.

use std::str::FromStr;

/// A member's rank inside one account.
///
/// Ordered Owner < Admin < Manager < Member by the derived [`Ord`], so a *lower*
/// numeric position means *higher* authority — [`can_grant`](Role::can_grant)
/// leans on this, so keep the variants in rank order, top to bottom.
///
/// References: [`UnknownRole`], [`crate::elements::user_account::UserAccount`],
/// [`crate::ports::AccountWrites::grant_role`], DESIGN/Roles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The account's founder/highest authority; never has a parent (DESIGN/Roles).
    Owner,
    /// May change roles below Admin; cannot mint a peer Admin or an Owner.
    Admin,
    /// A member with elevated standing but no authority to change roles.
    Manager,
    /// The base membership rank; grants nothing.
    Member,
}

impl Role {
    /// The stored/wire discriminant: `"owner" | "admin" | "manager" | "member"` —
    /// the one vocabulary list [`FromStr`] parses back.
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

/// A stored role discriminant that isn't one of the four known roles.
///
/// The error returned by [`Role::from_str`] when a persisted string doesn't map
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
        // keep it Owner→Member, top to bottom.
        matches!(self, Role::Owner | Role::Admin) && target > self
    }
}

/// A member's alias for their own role on an account — a free-form label (e.g.
/// an Owner aliased "Studio Head"), never a second authority axis: it lives on
/// the *membership* ([`crate::elements::user_account::UserAccount::alias`],
/// [`crate::elements::account::AccountMembership::alias`]), not on [`Role`]
/// itself, so it can never influence [`Role`]'s derived [`Ord`] or
/// [`can_grant`](Role::can_grant). Distinct from a member's *parent* — that is
/// the inviting member, stored in a separate `account_members.parent` column,
/// never here.
///
/// A custom constructor rather than bare `FromStr`/`TryFrom<String>`:
/// `RoleAlias::new` trims before validating (`FromStr::from_str` gets that for
/// free too, since it delegates), and there is no infallible `String` this
/// could sensibly implement `From` for — every `String` must still clear the
/// non-empty check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleAlias(String);

/// A role alias that failed validation — empty (after trimming), the only
/// rule a free-form label carries.
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
    /// no charset or length rule, unlike [`crate::elements::handle::Handle`].
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

    // The full actor → target matrix for `can_grant`, straight off DESIGN/Roles.
    // The e2e suite can only sign in the Owner, so the Admin/Manager/Member actor
    // rows live here — this is where the rule is pinned.
    #[test]
    fn can_grant_matrix_matches_the_design() {
        let roles = || [Role::Owner, Role::Admin, Role::Manager, Role::Member];
        for actor in roles() {
            for target in roles() {
                // An actor grants only roles strictly below its own rank, and only
                // Owner and Admin may grant at all (Manager and Member grant nothing).
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

    // The sharpest edges, stated outright: no one grants Owner through this seam
    // (transfer is its own path), an Admin cannot mint a peer Admin, and the lower
    // roles grant nothing at all.
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
