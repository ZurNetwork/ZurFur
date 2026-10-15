//! The Den's fixed words and the conversions every arm uses to name its nodes.

use std::fmt::Display;

use atta::{NodeName, Segment};
use domain::elements::{did::Did, text::StoredText};

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

/// The key of an entity keyed by a UUID-backed id: the id's text; `None` only if that text
/// breaks the segment rule, which a test rules out for every UUID.
pub(crate) fn id_segment(id: impl Display) -> Option<Segment> {
    Segment::try_from(id.to_string()).ok()
}

/// The key of an actor keyed by its DID; `None` for a DID longer than a segment allows, whose
/// node therefore has no key and is absent.
pub(crate) fn did_segment(did: &Did) -> Option<Segment> {
    Segment::try_from(did.as_ref()).ok()
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
