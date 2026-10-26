//! A commission's acts through the Den's write: each act's body sits beside its typed use case.

use std::fmt;

use crate::{
    commission::{
        create, deadline, lifecycle::move_to, rename, slots::declare, status::direction, sweep,
    },
    den::act::ActSpec,
};

/// Every commission act the Den's write carries, in the order the acts list shows them. Its
/// kind, [`CommissionActKind`], names each act without a body.
#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(
    name(CommissionActKind),
    derive(strum::VariantArray),
    doc = "Each commission act by its kind alone, in the acts list's order."
)]
pub(crate) enum CommissionAct {
    /// Creates a commission.
    Create(create::Act),
    /// Renames a commission.
    Rename(rename::Act),
    /// Moves a commission's lifecycle step.
    MoveLifecycle(move_to::Act),
    /// Sets a commission's direction status.
    SetDirection(direction::set::Act),
    /// Clears a commission's direction status.
    ClearDirection(direction::clear::Act),
    /// Sets or moves a commission's deadline.
    SetDeadline(deadline::set::Act),
    /// Clears a commission's deadline.
    ClearDeadline(deadline::clear::Act),
    /// Raises a commission's Delayed flag.
    SetDelayed(deadline::status::set::Act),
    /// Takes a commission's Delayed flag down.
    ClearDelayed(deadline::status::clear::Act),
    /// Declares one Slot on a commission.
    DeclareSlot(declare::Act),
    /// The system's Late mark on a commission whose deadline lapsed.
    MarkLate(sweep::Act),
}

impl CommissionAct {
    /// What this act declares about itself.
    pub(crate) fn spec(&self) -> ActSpec {
        match self {
            CommissionAct::Create(act) => act.spec(),
            CommissionAct::Rename(act) => act.spec(),
            CommissionAct::MoveLifecycle(act) => act.spec(),
            CommissionAct::SetDirection(act) => act.spec(),
            CommissionAct::ClearDirection(act) => act.spec(),
            CommissionAct::SetDeadline(act) => act.spec(),
            CommissionAct::ClearDeadline(act) => act.spec(),
            CommissionAct::SetDelayed(act) => act.spec(),
            CommissionAct::ClearDelayed(act) => act.spec(),
            CommissionAct::DeclareSlot(act) => act.spec(),
            CommissionAct::MarkLate(act) => act.spec(),
        }
    }
}

impl fmt::Debug for CommissionAct {
    /// The act's variant alone; never its body, which may carry a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant = match self {
            CommissionAct::Create(_) => "Create",
            CommissionAct::Rename(_) => "Rename",
            CommissionAct::MoveLifecycle(_) => "MoveLifecycle",
            CommissionAct::SetDirection(_) => "SetDirection",
            CommissionAct::ClearDirection(_) => "ClearDirection",
            CommissionAct::SetDeadline(_) => "SetDeadline",
            CommissionAct::ClearDeadline(_) => "ClearDeadline",
            CommissionAct::SetDelayed(_) => "SetDelayed",
            CommissionAct::ClearDelayed(_) => "ClearDelayed",
            CommissionAct::DeclareSlot(_) => "DeclareSlot",
            CommissionAct::MarkLate(_) => "MarkLate",
        };
        formatter.write_str(variant)
    }
}

#[cfg(test)]
mod tests;
