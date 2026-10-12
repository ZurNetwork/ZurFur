//! The Den's fixed words and the conversions every arm uses to name its nodes.

use atta::{NodeName, Segment};

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
#[allow(dead_code, reason = "the identity arms mount a user's posts under it")]
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

/// The name for stored text: the text itself when it passes the name rule, else `Unnamed`.
#[allow(
    dead_code,
    reason = "the identity and commission arms name stored nodes through it"
)]
pub(crate) fn stored_name(text: &str) -> Option<NodeName> {
    NodeName::try_from(text)
        .ok()
        .or_else(|| fixed_name(UNNAMED))
}

#[cfg(test)]
mod tests;
