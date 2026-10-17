//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates. Per-area ports live in submodules ([`commission`],
//! [`changelog`], …) and are re-exported flat here, so a new area adds a file
//! rather than growing this one.

mod unit_of_work;

pub use ::identity::ports::*;
pub use ::workflow::ports::*;
pub use unit_of_work::{Database, Unit, UnitOfWork, UnitOfWorkFn};
