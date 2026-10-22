//! The Den's fixed words and the conversions every arm uses to name its nodes.

use std::fmt::Display;

use atta::{DisplayName, Segment};
use domain::elements::{
    commission::{CommissionId, ElementId, FileKey},
    did::Did,
    name::NodeName,
    text::StoredText,
    workflow::WorkflowId,
};

/// A user's folder of Account memberships.
pub(crate) const ACCOUNTS: &str = "accounts";
/// A user's or Account's folder of Characters.
pub(crate) const CHARACTERS: &str = "characters";
/// A user's or Account's folder of commissions.
pub(crate) const COMMISSIONS: &str = "commissions";
/// An Account's folder of Workflows.
pub(crate) const WORKFLOWS: &str = "workflows";
/// A commission's folder of attachments.
pub(crate) const ATTACHMENTS: &str = "attachments";
/// A commission's changelog file.
pub(crate) const CHANGELOG: &str = "changelog";
/// A commission's folder of products.
pub(crate) const PRODUCTS: &str = "products";
/// A commission's folder of slots.
pub(crate) const SLOTS: &str = "slots";
/// A user's mount of their own posts.
pub(crate) const POSTS: &str = "posts";
/// The name a node shows when its stored text fails the name rule.
pub(crate) const UNNAMED: &str = "Unnamed";

/// A fixed word as a segment; `None` only if the word breaks the segment rule, which a test
/// rules out for every word.
pub(crate) fn fixed_segment(word: &'static str) -> Option<Segment> {
    Segment::try_from(word).ok()
}

/// A fixed word as a name; `None` only if the word breaks the name rule, which a test rules
/// out for every word.
pub(crate) fn fixed_name(word: &'static str) -> Option<NodeName> {
    NodeName::try_from(word).ok()
}

/// A typed id backed by a UUID. A den trait rather than an `AsRef<Uuid>` bound because this
/// crate doesn't depend on `uuid`; implemented only for the typed ids, so request text never
/// becomes a key.
pub(crate) trait UuidKey: Display {}

impl UuidKey for CommissionId {}
impl UuidKey for ElementId {}
impl UuidKey for FileKey {}
impl UuidKey for WorkflowId {}
impl<T: UuidKey> UuidKey for &T {}

/// The key of an entity keyed by a UUID-backed id: the id's text; `None` only if that text
/// breaks the segment rule, which a test rules out for every UUID.
pub(crate) fn id_segment(id: impl UuidKey) -> Option<Segment> {
    Segment::try_from(id.to_string()).ok()
}

/// The key of an actor keyed by its DID; `None` for a DID longer than a segment allows, whose
/// node therefore has no key and is absent.
pub(crate) fn did_segment(did: &Did) -> Option<Segment> {
    Segment::try_from(did.as_ref()).ok()
}

/// The engine's display name for a name the Den's rule passed; the one place a name enters
/// the engine. A free function, not `From`: both types are foreign to this crate.
#[expect(
    clippy::disallowed_methods,
    reason = "the one construction of the engine's display name, from a name the rule passed"
)]
pub(crate) fn display_name(name: &NodeName) -> DisplayName {
    DisplayName::host_checked(name.to_string())
}

/// The name for stored text: the text itself when it passes the name rule, else `Unnamed`.
pub(crate) fn stored_name(text: &str) -> Option<NodeName> {
    NodeName::try_from(text)
        .ok()
        .or_else(|| fixed_name(UNNAMED))
}

/// Where a node's name comes from: a fixed word, or text a store holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Name {
    /// One of the Den's fixed words.
    Fixed(&'static str),
    /// Text as the store holds it, shown as `Unnamed` when it fails the name rule.
    Stored(StoredText),
}

impl Name {
    /// The node's name; `None` only if a fixed word breaks the name rule, which a test rules
    /// out for every word.
    pub(crate) fn into_node_name(self) -> Option<NodeName> {
        match self {
            Name::Fixed(word) => fixed_name(word),
            Name::Stored(text) => stored_name(text.as_str()),
        }
    }
}

#[cfg(test)]
mod tests;
