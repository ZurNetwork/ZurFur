//! A Den node: its key, [`NodeId`]; its path segment, [`NodeSegment`]; its closed
//! type, [`NodeType`]; and the rows that cross the node port, [`NewNode`] and [`NodeRow`].

mod errors;
mod id;
mod node_type;
mod rows;
mod values;

pub use errors::NodeSegmentError;
pub use id::NodeId;
pub use node_type::{NodeType, UnknownNodeType};
pub use rows::{NewNode, NodeRow};
pub use values::{MAX_SEGMENT_BYTES, NodeSegment};
