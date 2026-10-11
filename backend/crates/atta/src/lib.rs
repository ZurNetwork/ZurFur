//! Atta: the engine for a tree of typed files. It resolves the tree,
//! enforces its structural rules, routes each operation to the handler for
//! a node's type, and renders files.
//!
//! Types and content are opaque to it. It knows nothing about the
//! application that embeds it.

mod level;
mod metadata;
mod name;
mod path;
mod segment;

pub use level::Level;
pub use metadata::{Metadata, Type};
pub use name::{MAX_NAME_BYTES, NodeName, NodeNameError};
pub use path::RealPath;
pub use segment::{MAX_SEGMENT_BYTES, Segment, SegmentError};
