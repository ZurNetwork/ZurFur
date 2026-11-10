//! Kernel ports: traits named by the role they play for the domain, implemented by
//! the adapter crates, the write capability a gated port method will take, and key
//! custody with its purpose-scoped handles. Each area's ports live in a submodule and
//! are re-exported flat here.

mod capability;
mod custody;
mod errors;
mod node;

pub use capability::{Creator, New, WriteCapability};
pub use custody::{KeyCustodian, ScopedMac, ScopedSealer};
pub use errors::SegmentTaken;
pub use node::{NodeReads, NodeRepo, NodeStore, NodeWrites};
