//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates. The kernel, identity and workflow ports are re-exported flat
//! here (per-area submodules such as [`commission`] and [`changelog`] included);
//! the unit of work lives here because it vends the write ports of all three.

mod unit_of_work;

pub use ::identity::ports::*;
pub use ::kernel::ports::*;
pub use ::workflow::ports::*;
pub use unit_of_work::{Database, Unit, UnitOfWork, UnitOfWorkFn};
