//! A commission's acts through the Den's write: each act's body sits beside its typed use case.

use std::fmt;

use crate::{commission::create, den::act::ActSpec};

/// Every commission act the Den's write carries.
pub(crate) enum CommissionAct {
    /// Creates a commission.
    Create(create::Act),
}

impl CommissionAct {
    /// What this act declares about itself.
    pub(crate) fn spec(&self) -> ActSpec {
        match self {
            CommissionAct::Create(act) => act.spec(),
        }
    }
}

impl fmt::Debug for CommissionAct {
    /// The act's variant alone; never its body, which carries a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommissionAct::Create(_) => formatter.write_str("Create"),
        }
    }
}

#[cfg(test)]
mod tests;
