//! [`GolemId`] — the identity of a Golem. **Stub.**
//!
//! A Golem is a non-human *participant* — an actor with a seat at the table, not
//! machinery (DESIGN/Golem). Owned by an [`crate::elements::account::Account`],
//! it is explicitly added to a commission, holds a commission-scoped role, and
//! acts **as itself** (its own principal), bounded by role ∩ scopes ∩ domain
//! validation. Deliberately *not* an AI agent: it does only what its role and
//! instructions permit, and its driver is hosted externally (Telegram-bot
//! style). Only the id type exists so far; the Golem entity is not modelled here
//! yet. A Golem can sit in a commission as a
//! [`crate::elements::commission::ParticipantRef::Golem`].

/// The identity of a Golem.
///
/// Stub: a UUIDv7 wrapped for type safety; the Golem entity itself is not
/// modelled here yet. See [`crate::elements::commission::ParticipantRef`] for
/// where a Golem takes part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GolemId(uuid::Uuid);
