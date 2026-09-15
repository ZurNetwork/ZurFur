use crate::elements::{commission::element::ElementId, user::UserId};

use super::{SeatKind, SeatLink, SeatPrompt};

/// One stored Seat as read back
/// ([`CommissionStore::seats`](crate::ports::CommissionStore::seats)) — the
/// interpreted satellite half; the element half lives in the loaded composition
/// under the same id. `occupant` is the whole occupancy model: one `Option`, so
/// more than one occupant is unrepresentable.
#[derive(Debug)]
pub struct Seat {
    /// The seat's identity: its carrying element's id (the satellite key).
    pub id: ElementId,
    /// The seat's semantic kind.
    pub kind: SeatKind,
    /// The free-text requirement prompt, if the vacant seat carries one.
    pub prompt: Option<SeatPrompt>,
    /// The external requirements link, if the vacant seat carries one.
    pub link: Option<SeatLink>,
    /// The single occupant slot: `None` while vacant.
    pub occupant: Option<UserId>,
}

impl Seat {
    /// Whether the seat is unoccupied.
    pub fn is_vacant(&self) -> bool {
        self.occupant.is_none()
    }
}

#[cfg(test)]
mod tests;
