//! [`CharacterId`] — the identity of a Character. **Stub.**
//!
//! A Character is a repository and representation of a character, kept by one or
//! more **Keepers** (a single Keeper in v1) and identified by its own `did:plc`
//! (DESIGN/Character). Characters are sovereign data: they have their own PDS,
//! their own username and profile (distinct from the Keeper's), and are meant to
//! survive both the user that created them and account deletion. A Character can
//! be *assigned* to a slot in a commission
//! (see [`crate::elements::commission::Slot`]); only one occupies a slot at a
//! time. Only the id type exists so far; the Character entity is not modelled
//! here yet.

/// The app-private identity of a Character.
///
/// Stub: a UUIDv7 wrapped for type safety. Note the Character's *public*
/// identity is a [`crate::elements::did::Did`] (per DESIGN/Character); this is
/// the private handle. The entity itself is not modelled here yet. Referenced
/// from a commission slot via
/// [`crate::elements::commission::Slot::character_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharacterId(uuid::Uuid);
