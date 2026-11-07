//! The [`Invitation`]: a pending offer of account membership — the issuing half
//! of invite-then-accept. Joining someone else's account is consequential, so it
//! is consensual: an Owner or Admin issues the offer and the invited User must
//! accept before any membership exists.
//!
//! Authority to issue is the grant rule
//! ([`Role::can_grant`](crate::elements::role::Role::can_grant)); the inviter is
//! recorded because on acceptance they become the new member's Parent. There is
//! no expiry. An offer stays valid only while its issuer is a member whose role
//! may still offer its role.

mod entity;
mod errors;
mod id;
mod state;

pub use entity::Invitation;
pub use errors::{InvitationError, UnknownInvitationState};
pub use id::InvitationId;
pub use state::InvitationState;
