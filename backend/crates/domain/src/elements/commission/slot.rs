//! The commission's Slots: declared Character positions — a commission may
//! define them, title them, count them — whose filling is deferred to the
//! Character epic.
//!
//! Declaring one contributes an ordinary element typed
//! [`ElementType::slot`](super::ElementType::slot), with the title and notes in
//! a satellite row keyed by that element's id. Fill is unrepresentable: no shape
//! here carries an occupant, and an empty Slot is a valid permanent state.

use std::str::FromStr;

use super::{
    CommissionId,
    element::{ElementId, SurfaceAddress},
};
use crate::{
    datetime::DateTimeUtc,
    elements::user::UserId,
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// A Slot's title — the one required facet of a declared Slot: trimmed, and
/// non-empty. No length cap yet.
///
/// ```
/// use domain::elements::commission::SlotTitle;
///
/// let title = "  The knight  ".parse::<SlotTitle>().unwrap();
/// assert_eq!(title.as_str(), "The knight"); // trimmed
///
/// assert!("   ".parse::<SlotTitle>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotTitle(String);

/// Why a string was rejected as a Slot title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotTitleError {
    /// Empty once trimmed.
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

/// Parses a Slot title: trimmed, then non-empty or [`SlotTitleError::Empty`].
impl FromStr for SlotTitle {
    type Err = SlotTitleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StringBuilder::new(s)
            .trimmed()
            .non_empty()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => SlotTitleError::Empty,
                StringBuilderViolation::TooLong { .. }
                | StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain only applies trimmed().non_empty().
                    debug_assert!(
                        false,
                        "SlotTitle's FromStr chain only applies trimmed().non_empty()"
                    );
                    SlotTitleError::Empty
                }
            })
    }
}

impl AsRef<str> for SlotTitle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl SlotTitle {
    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A freshly declared Slot, ready to persist into a declared surface
/// ([`CommissionWrites::declare_slots`](crate::ports::CommissionWrites::declare_slots)).
/// The store writes an ordinary element and the Slot satellite beside it, keyed
/// by that element's id. No occupant field of any kind.
#[derive(Debug)]
pub struct NewSlot {
    /// The element key (UUIDv7) of the element carrying this Slot; it also keys
    /// the satellite row.
    pub id: ElementId,
    /// The commission this Slot is declared on.
    pub commission_id: CommissionId,
    /// Where the carrying element sits: the (tab, surface) pair. An absent or
    /// foreign tab refuses with [`UnknownTab`](crate::ports::UnknownTab); an
    /// undeclared pair with [`UnknownSurface`](crate::ports::UnknownSurface).
    pub address: SurfaceAddress,
    /// The Slot's required title, validated at the boundary.
    pub title: SlotTitle,
    /// Optional freeform notes, carried verbatim; the boundary trims and maps
    /// blank to `None`.
    pub notes: Option<String>,
    /// The acting User.
    pub created_by: UserId,
    /// When the Slot was declared.
    pub created_at: DateTimeUtc,
}

impl NewSlot {
    /// A new Slot contributed at `address`, titled `title`, with optional
    /// `notes`. Mints the element id; authority, the tab's existence and the
    /// surface's declaration are settled on persist.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSlot, SlotTitle, SurfaceAddress, SurfaceName, TabId},
    ///     did::Did,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::new(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let owner = UserId::new(Did::new("did:plc:alice".to_string()));
    /// let title = "The knight".parse::<SlotTitle>().unwrap();
    /// let slot = NewSlot::contributed_at(commission, address.clone(), title, None, owner, Utc::now());
    /// assert_eq!(slot.address, address);
    /// assert_eq!(slot.title.as_str(), "The knight");
    /// assert!(slot.notes.is_none());
    /// ```
    pub fn contributed_at(
        commission: CommissionId,
        address: SurfaceAddress,
        title: SlotTitle,
        notes: Option<String>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: ElementId::mint(),
            commission_id: commission,
            address,
            title,
            notes,
            created_by,
            created_at: now,
        }
    }
}

/// One declared Slot as read back — the satellite row (title, notes) plus the
/// element id that keys it. Occupant-less: an empty Slot is the complete,
/// permanent v1 state, not a Slot waiting on anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The id of the element carrying this Slot — also the satellite row's key.
    pub element_id: ElementId,
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
    use crate::elements::did::Did;

    // The title is trimmed on the way in, and a blank one is refused.
    #[test]
    fn a_slot_title_trims_and_rejects_blank() {
        assert_eq!(
            "  The knight  ".parse::<SlotTitle>().unwrap().as_str(),
            "The knight"
        );
        assert_eq!("".parse::<SlotTitle>(), Err(SlotTitleError::Empty));
        assert_eq!("   \t ".parse::<SlotTitle>(), Err(SlotTitleError::Empty));
    }

    // A new Slot's envelope: fresh id, address, acting user, title, notes.
    #[test]
    fn a_new_slot_carries_title_and_optional_notes() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let address = SurfaceAddress::new(
            super::super::element::TabId::new(uuid::Uuid::now_v7()),
            "content".parse().unwrap(),
        );
        let owner = UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())));
        let title = "The mage".parse::<SlotTitle>().unwrap();

        let slot = NewSlot::contributed_at(
            commission,
            address.clone(),
            title.clone(),
            Some("robes, not armor".to_string()),
            owner.clone(),
            Utc::now(),
        );

        assert_eq!(slot.commission_id, commission);
        assert_eq!(slot.address, address);
        assert_eq!(slot.title, title);
        assert_eq!(slot.notes.as_deref(), Some("robes, not armor"));
        assert_eq!(slot.created_by, owner);
    }
}
