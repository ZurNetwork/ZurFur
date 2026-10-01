use super::errors::UnknownRole;

/// A member's rank inside one account. The derived [`Ord`] runs Owner < Admin <
/// Manager < Member, so a lower position means higher authority and
/// [`can_grant`](Role::can_grant) leans on it — **keep the variants in rank
/// order**.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    ascii_case_insensitive,
    parse_err_ty = UnknownRole,
    parse_err_fn = unknown_token
)]
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

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(token: &str) -> UnknownRole {
    UnknownRole(token.into())
}

impl Role {
    pub fn is_administrative(&self) -> bool {
        matches!(self, Role::Owner | Role::Admin)
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
