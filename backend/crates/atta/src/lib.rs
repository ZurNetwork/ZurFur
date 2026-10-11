//! Atta: the engine for a tree of typed files. It resolves the tree,
//! enforces its structural rules, routes each operation to the handler for
//! a node's type, and renders files.
//!
//! Types and content are opaque to it. It knows nothing about the
//! application that embeds it, which it reaches through one [`Host`].

mod host;
mod level;
mod list;
mod metadata;
mod name;
mod page;
mod path;
mod resolve;
mod segment;

#[cfg(test)]
mod fake;

pub use host::{
    Access, Admission, Found, Host, HostError, Kind, Lift, Mount, Reach, Relationship, Scope, Via,
    Viewer,
};
pub use level::Level;
pub use metadata::{Metadata, Type};
pub use name::{MAX_NAME_BYTES, NodeName, NodeNameError};
pub use page::{Listing, PageToken, PageTokenError};
pub use path::RealPath;
pub use resolve::{Caps, Entry, Error, Resolved, Resolver, View};
pub use segment::{MAX_SEGMENT_BYTES, Segment, SegmentError};
