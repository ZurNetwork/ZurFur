//! The [`ActorIdentity`] — a row in the actor super-table: one row per actor the
//! Index has ever seen, and the single table every actor reference FKs into.
//!
//! Rows are immortal: the port exposes no delete, so liveness is an
//! [`ActorState`] on the row and an FK into `actor_identity` can never break.
//! [`ActorKind`] is the closed vocabulary that, with `UNIQUE (id, kind)`, every
//! kind-checked reference's composite FK targets. A row's `handle` is a
//! refreshable display cache, never a claim-validated handle, and `first_seen`
//! is immutable.

mod entity;
mod errors;
mod id;
mod kind;
mod state;

pub use entity::ActorIdentity;
pub use errors::{UnknownActorKind, UnknownActorState};
pub use id::ActorIdentityId;
pub use kind::ActorKind;
pub use state::ActorState;
