use crate::elements::commission::{CommissionId, element::ElementId};

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
    pub title: super::SlotTitle,
    /// The optional freeform notes, exactly as declared.
    pub notes: Option<String>,
}
