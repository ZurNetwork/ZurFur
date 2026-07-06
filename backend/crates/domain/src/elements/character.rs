//! [`CharacterId`] — the identity of a Character. **Stub.**
//!
//! A Character is a repository and representation of a character, kept by one or
//! more **Keepers** (a single Keeper in v1) and identified by its own `did:plc`
//! (DESIGN/Character). Characters are sovereign data: they have their own PDS,
//! their own username and profile (distinct from the Keeper's), and are meant to
//! survive both the user that created them and account deletion. A Character
//! will be *assigned* to a declared commission Slot
//! (see [`crate::elements::commission::Slot`]) — one occupant per Slot — but
//! that assignment surface is this epic's to build: ZMVP-77 deliberately left
//! [`Slot`](crate::elements::commission::Slot) occupant-less (fill
//! unrepresentable, not just unoffered). Only the id type exists so far; the
//! Character entity is not modelled here yet.

/// The app-private identity of a Character.
///
/// Stub: a UUIDv7 wrapped for type safety. Note the Character's *public*
/// identity is a [`crate::elements::did::Did`] (per DESIGN/Character); this is
/// the private handle. The entity itself is not modelled here yet; the
/// commission [`Slot`](crate::elements::commission::Slot) it will occupy gains
/// its occupant reference in the same change that models assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharacterId(uuid::Uuid);
