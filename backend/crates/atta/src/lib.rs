//! Atta: the engine for a tree of typed files. It resolves the tree,
//! enforces its structural rules, routes each operation to the handler for
//! a node's type, and renders files. A change is a named act: Atta checks
//! its generic rules, then hands it to the host to apply.
//!
//! Types and content are opaque to it. It knows nothing about the
//! application that embeds it, which it reaches through one [`Host`].

mod act;
mod host;
mod level;
mod list;
mod metadata;
mod ordering;
mod ordinal;
mod page;
mod path;
mod resolve;
mod segment;
mod write;

#[cfg(test)]
mod fake;

pub use act::{Act, ActName, Born, Placed, Shape};
pub use host::{
    Access, Admission, Found, Host, HostError, Kind, Lift, Mount, Reach, Relationship, Scope, Via,
    Viewer,
};
pub use level::Level;
pub use metadata::{DisplayName, Metadata, Type};
pub use ordinal::{NameOrdinal, NameOrdinalError};
pub use page::{Listing, PageToken, PageTokenError};
pub use path::RealPath;
pub use resolve::{Caps, Entry, Error, Resolved, Resolver, View};
pub use segment::{MAX_SEGMENT_BYTES, Segment, SegmentError};
pub use write::{Checked, Refusal, WriteError, Writer};
