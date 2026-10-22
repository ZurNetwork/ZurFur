//! A commission's acts through the Den's write: each act's body sits beside its typed use case.

use std::fmt;

use crate::{
    commission::{create, lifecycle::move_to, rename},
    den::act::ActSpec,
};

/// Every commission act the Den's write carries.
pub(crate) enum CommissionAct {
    /// Creates a commission.
    Create(create::Act),
    /// Renames a commission.
    Rename(rename::Act),
    /// Moves a commission's lifecycle step.
    MoveLifecycle(move_to::Act),
}

impl CommissionAct {
    /// What this act declares about itself.
    pub(crate) fn spec(&self) -> ActSpec {
        match self {
            CommissionAct::Create(act) => act.spec(),
            CommissionAct::Rename(act) => act.spec(),
            CommissionAct::MoveLifecycle(act) => act.spec(),
        }
    }
}

impl fmt::Debug for CommissionAct {
    /// The act's variant alone; never its body, which may carry a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommissionAct::Create(_) => formatter.write_str("Create"),
            CommissionAct::Rename(_) => formatter.write_str("Rename"),
            CommissionAct::MoveLifecycle(_) => formatter.write_str("MoveLifecycle"),
        }
    }
}

#[cfg(test)]
mod tests;
