//! The [`SeatInvitation`]: a pending offer of a commission Seat — the issuing
//! half of seat invite-then-accept. Filling a vacant Seat is consensual, so the
//! owner issues an offer and the invited User must accept.
//!
//! Reuses the account invitation's [`InvitationState`] machine wholesale. This
//! module only issues a pending offer or revokes it; the accepted transition and
//! the occupancy it mints live elsewhere.

mod entity;
mod id;

pub use entity::SeatInvitation;
pub use id::SeatInvitationId;
