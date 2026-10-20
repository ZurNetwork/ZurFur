//! [`Role`] — a member's rank inside an account, and the rule for who may grant
//! what. Four ranks, Owner highest; only Owner and Admin may change roles, and
//! the grant rule itself lives in [`Role::can_grant`].

mod alias;
mod errors;
mod rank;

pub use alias::RoleAlias;
pub use errors::{InvalidRoleAlias, UnknownRole};
pub use rank::Role;
