//! Kernel ports: traits named by the role they play for the domain, implemented by
//! the adapter crates. Each area's ports live in a submodule and are re-exported
//! flat here.

mod errors;
mod node;

pub use errors::SegmentTaken;
pub use node::{NodeReads, NodeRepo, NodeStore, NodeWrites};
