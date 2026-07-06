//! The commission's **Slots** (ZMVP-77; DESIGN/Slots `5931025`, Referenceable,
//! Slot & Seat DD `28311564`): declared Character positions — a commission may
//! define them, title them, count them — whose *filling* is deferred wholesale
//! to the Character epic.
//!
//! A Slot is a **component in the tree** (ruling: the slot's substance — a
//! required title, optional freeform notes — lives in a satellite
//! `commission_slot` table keyed by the slot node's id, mirroring the Seat
//! satellite of Gate A ruling E20; the node itself is an ordinary
//! [`NodeKind::Component`] leaf). "Deliberately not Participants" stands: a
//! Slot holds a Character, never a User, so nothing here touches seats, roles,
//! or the participant set.
//!
//! **Fill is unrepresentable, not just unoffered** (AC3): neither [`NewSlot`]
//! nor the read-back [`Slot`] carries any occupant/character field, no port
//! writes one, and the satellite table has no column for one — an empty Slot is
//! a valid, *permanent* state (AC2; nothing expires or auto-fills it). The
//! assignment surface (public-vs-private character gates, live reference)
//! arrives with the Character epic.
//!
//! [`NodeKind::Component`]: super::NodeKind::Component

use super::{CommissionId, node::NodeId};
use crate::{datetime::DateTimeUtc, elements::user::UserId};

/// A Slot's title, validated on the way in — the one **required** facet of a
/// declared Slot (ZMVP-77 AC1).
///
/// Surrounding whitespace is trimmed; the result must be non-empty — the same
/// construction-time gate [`CommissionTitle`](super::CommissionTitle) applies to
/// commission titles (and like it, no length cap is imposed here yet).
///
/// ```
/// use domain::elements::commission::SlotTitle;
///
/// let title = SlotTitle::try_from("  The knight  ").unwrap();
/// assert_eq!(title.as_str(), "The knight"); // trimmed
///
/// assert!(SlotTitle::try_from("   ").is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotTitle(String);

/// Why a string was rejected as a Slot title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotTitleError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
}

impl std::fmt::Display for SlotTitleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotTitleError::Empty => write!(f, "slot title must not be empty"),
        }
    }
}

impl std::error::Error for SlotTitleError {}

impl TryFrom<String> for SlotTitle {
    type Error = SlotTitleError;

    /// Validate and wrap a title: trim surrounding whitespace, then reject an
    /// empty result with [`SlotTitleError::Empty`].
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let trimmed = raw.trim().to_owned();
        if trimmed.is_empty() {
            return Err(SlotTitleError::Empty);
        }
        Ok(Self(trimmed))
    }
}

impl TryFrom<&str> for SlotTitle {
    type Error = SlotTitleError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

impl SlotTitle {
    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A freshly declared Slot, ready to persist under an existing **surface**
/// ([`CommissionWrites::declare_slots`](crate::ports::CommissionWrites::declare_slots),
/// ZMVP-77).
///
/// Built with [`NewSlot::under`]. The tree half is exactly a
/// [`NewComponent`](super::NewComponent)'s envelope (the store persists an
/// ordinary component leaf — no mode, append sibling order, the empty payload);
/// the slot half — the required [`SlotTitle`] and optional freeform notes — is
/// the satellite the store persists beside it, keyed by this same node id.
/// There is deliberately **no occupant field of any kind**: fill is the
/// Character epic's, and an undeclarable field can't be filled by accident.
#[derive(Debug)]
pub struct NewSlot {
    /// The freshly minted node key (UUIDv7) — the slot node's id, which also
    /// keys the satellite row.
    pub id: NodeId,
    /// The commission whose tree this grows. The store verifies `parent`
    /// belongs to this same commission.
    pub commission_id: CommissionId,
    /// The existing **surface** to grow under. Slots are components — leaves —
    /// so the store refuses a parent that is itself a component
    /// ([`ParentNotASurface`](crate::ports::ParentNotASurface)).
    pub parent: NodeId,
    /// The Slot's required title (AC1), validated at the boundary.
    pub title: SlotTitle,
    /// Optional freeform notes (AC1) — carried verbatim; the boundary trims and
    /// normalizes blank to `None` before this is built.
    pub notes: Option<String>,
    /// The acting User (the owner; the route's authority gate settles that
    /// before this is built).
    pub created_by: UserId,
    /// When the Slot was declared.
    pub created_at: DateTimeUtc,
}

impl NewSlot {
    /// A new Slot under `parent`, titled `title`, with optional `notes`. Mints
    /// the node id; authority (owner-only in v1) and the parent-is-a-surface
    /// rule are the store's/route's concern, settled when this is persisted.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSlot, NodeId, SlotTitle},
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let parent = NodeId::new(uuid::Uuid::now_v7());
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let title = SlotTitle::try_from("The knight").unwrap();
    /// let slot = NewSlot::under(commission, parent, title, None, owner, Utc::now());
    /// assert_eq!(slot.parent, parent);
    /// assert_eq!(slot.title.as_str(), "The knight");
    /// assert!(slot.notes.is_none());
    /// ```
    pub fn under(
        commission: CommissionId,
        parent: NodeId,
        title: SlotTitle,
        notes: Option<String>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: NodeId::mint(),
            commission_id: commission,
            parent,
            title,
            notes,
            created_by,
            created_at: now,
        }
    }
}

/// One **declared** Slot as read back — the satellite row rebuilt (title,
/// notes) plus the node id that keys it. A commission holds zero or more of
/// these (AC2).
///
/// Deliberately occupant-less: an empty Slot is not a Slot *waiting* on
/// anything — it is the complete, permanent v1 state (AC2/AC3). When the
/// Character epic lands the assignment surface, the occupant joins this shape
/// (and its storage) in that change, not before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The slot node's id — the tree component this Slot decorates, and the
    /// satellite row's key.
    pub node_id: NodeId,
    /// The commission the Slot belongs to.
    pub commission_id: CommissionId,
    /// The Slot's required title.
    pub title: SlotTitle,
    /// The optional freeform notes, exactly as declared.
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    // AC1 — the title is required and validated: trimmed on the way in, a
    // blank one refused rather than stored.
    #[test]
    fn a_slot_title_trims_and_rejects_blank() {
        assert_eq!(
            SlotTitle::try_from("  The knight  ").unwrap().as_str(),
            "The knight"
        );
        assert_eq!(SlotTitle::try_from(""), Err(SlotTitleError::Empty));
        assert_eq!(SlotTitle::try_from("   \t "), Err(SlotTitleError::Empty));
    }

    // AC1 — a new Slot's envelope: fresh id, the surface it grows under, the
    // acting user, its title and optional notes carried as given.
    #[test]
    fn a_new_slot_carries_title_and_optional_notes() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let parent = NodeId::new(uuid::Uuid::now_v7());
        let owner = UserId::new(uuid::Uuid::now_v7());
        let title = SlotTitle::try_from("The mage").unwrap();

        let slot = NewSlot::under(
            commission,
            parent,
            title.clone(),
            Some("robes, not armor".to_string()),
            owner,
            Utc::now(),
        );

        assert_eq!(slot.commission_id, commission);
        assert_eq!(slot.parent, parent);
        assert_eq!(slot.title, title);
        assert_eq!(slot.notes.as_deref(), Some("robes, not armor"));
        assert_eq!(slot.created_by, owner);
    }
}
