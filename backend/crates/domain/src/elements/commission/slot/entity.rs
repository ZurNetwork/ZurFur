use crate::elements::{
    commission::{CommissionId, element::ElementId},
    text::StoredText,
};

/// One declared Slot as read back — the satellite row (title, notes) plus the
/// element id that keys it. Occupant-less: an empty Slot is the complete,
/// permanent v1 state, not a Slot waiting on anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The id of the element carrying this Slot — also the satellite row's key.
    pub element_id: ElementId,
    /// The commission the Slot belongs to.
    pub commission_id: CommissionId,
    /// The Slot's title as stored; it passed the title rule in force when it
    /// was declared, and a load never re-checks it.
    pub title: StoredText,
    /// The optional freeform notes, exactly as declared.
    pub notes: Option<String>,
}
