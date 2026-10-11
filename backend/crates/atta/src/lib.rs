//! Atta: the engine for a tree of typed files. It resolves the tree,
//! enforces its structural rules, routes each operation to the handler for
//! a node's type, and renders files.
//!
//! Types and content are opaque to it. It knows nothing about the
//! application that embeds it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A node's key, unique within its real parent.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Into,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct Key(uuid::Uuid);

/// A node's type; opaque to Atta.
pub enum Type {}

/// How visible a node is: absent, a card that can't be opened, or open.
pub enum Level {
    /// Absent.
    Private,
    /// A card that can't be opened.
    Listed,
    /// Open.
    Public,
}

/// A node's human-readable display name, never its key.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Into,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct NodeName(String);

/// The CID of a node's payload.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Into,
    derive_more::Display,
)]
pub struct Hash(cid::Cid);
/// The fixed fields Atta keeps on every node, the same for every type.
pub struct Metadata {
    pub key: Key,
    pub parent: Option<Key>,
    pub typ: Type,
    pub level: Level,
    pub name: NodeName,
    pub symlink_target: Option<Key>,
    pub hash: Hash,
}

/// A node's type-specific data, opaque to Atta.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, derive_more::From, derive_more::Into,
)]
pub struct Payload(Vec<u8>);

/// A node's data: its metadata, its payload and its attributes.
pub trait Node {
    /// The node's metadata.
    fn metadata(&self) -> &Metadata;
    /// The node's payload.
    fn payload(&self) -> &Payload;
    /// The node's attributes, as name and value pairs.
    fn attributes(&self) -> &HashMap<String, String>;
}
