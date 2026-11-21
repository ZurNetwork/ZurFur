use std::fmt;

use crate::elements::commission::element::{ElementId, SurfaceAddress};
use identity::elements::user::UserId;
use kernel::{datetime::DateTimeUtc, elements::node::NodeId};

/// What a declare chooses of a new Slot: where its carrying element sits and its notes. Its
/// title is its node's name, and its commission is its capability's.
#[derive(Clone, PartialEq, Eq)]
pub struct NewSlot {
    /// Where the carrying element sits: the (tab, surface) pair. An absent or foreign tab
    /// refuses with [`UnknownTab`](crate::ports::UnknownTab); an undeclared pair with
    /// [`UnknownSurface`](crate::ports::UnknownSurface).
    pub address: SurfaceAddress,
    /// Optional freeform notes, carried verbatim; the boundary trims and maps blank to `None`.
    pub notes: Option<String>,
}

/// What the den's write gate supplies to a Slot's declare, beside its capability: the element
/// it is born as, the node the gate made for it, who declared it and when.
#[derive(Clone, PartialEq, Eq)]
pub struct SlotBirth {
    /// The id of the element that carries the Slot; also its node's segment.
    pub element: ElementId,
    /// The Den node the Slot stands at.
    pub node: NodeId,
    /// The User who declared it.
    pub created_by: UserId,
    /// When it was declared.
    pub created_at: DateTimeUtc,
}

/// One Slot's declaration as its act carries it: the element it is born as and what the declare
/// chose.
#[derive(Clone, PartialEq, Eq)]
pub struct SlotDeclaration {
    /// The id of the element that will carry the Slot; also its node's segment.
    pub element: ElementId,
    /// What the declare chose.
    pub slot: NewSlot,
}

impl SlotDeclaration {
    /// `slot`'s declaration under a freshly minted element id (UUIDv7). Named, not `From`:
    /// it mints.
    pub fn minted(slot: NewSlot) -> Self {
        Self {
            element: ElementId::mint(),
            slot,
        }
    }
}

impl From<SlotDeclaration> for NewSlot {
    /// What the declaration chose, without the element it is born as.
    fn from(declaration: SlotDeclaration) -> Self {
        declaration.slot
    }
}

impl fmt::Debug for NewSlot {
    /// Its address; never its notes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewSlot")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SlotBirth {
    /// Its element, node and time; never who declared it, whose id is a DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SlotBirth")
            .field("element", &self.element)
            .field("node", &self.node)
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SlotDeclaration {
    /// Its element and its slot's Debug; never its notes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SlotDeclaration")
            .field("element", &self.element)
            .field("slot", &self.slot)
            .finish()
    }
}

#[cfg(test)]
mod tests;
