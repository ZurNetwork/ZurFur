use std::str::FromStr;

use domain::elements::name::{NodeName, NodeNameError, ShowableNameError};

/// A node's display name: text the Den's name rule accepts. Parsed from user-typed text on
/// every write that names a node, and re-checked for every node a read shows.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef, derive_more::From,
)]
#[as_ref(str)]
pub struct DenName(NodeName);

/// Why a piece of text is not a [`DenName`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DenNameError {
    /// Nothing is left once blank characters are stripped.
    #[error("a name is empty")]
    Empty,
    /// The name is longer than the Den allows.
    #[error("a name is too long")]
    TooLong,
    /// The name holds a `/`.
    #[error("a name holds a `/`")]
    Slash,
    /// The name holds a control or format character the rule doesn't keep.
    #[error("a name holds a control or format character")]
    Invisible,
    /// The name holds a line or paragraph separator.
    #[error("a name holds a line or paragraph separator")]
    Separator,
    /// The name holds a noncharacter or an object replacement character.
    #[error("a name holds a noncharacter or an object replacement character")]
    Reserved,
}

/// The name rule's refusal, named in this crate's own terms.
fn refusal(error: NodeNameError) -> DenNameError {
    match error {
        NodeNameError::Empty => DenNameError::Empty,
        NodeNameError::TooLong => DenNameError::TooLong,
        NodeNameError::Slash => DenNameError::Slash,
        NodeNameError::Invisible => DenNameError::Invisible,
        NodeNameError::Separator => DenNameError::Separator,
        NodeNameError::Reserved => DenNameError::Reserved,
    }
}

/// The refusal of a name safe to show, named in this crate's own terms: a name that
/// may hold a `/` refuses for the same reasons a node's name does, but the `/`.
pub(crate) fn showable_refusal(error: ShowableNameError) -> DenNameError {
    refusal(NodeNameError::from(error))
}

impl TryFrom<String> for DenName {
    type Error = DenNameError;

    /// The Den's name rule, stripping what it strips.
    fn try_from(text: String) -> Result<Self, Self::Error> {
        let name = NodeName::try_from(text).map_err(refusal)?;
        Ok(Self(name))
    }
}

impl TryFrom<&str> for DenName {
    type Error = DenNameError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        let name = NodeName::try_from(text).map_err(refusal)?;
        Ok(Self(name))
    }
}

impl FromStr for DenName {
    type Err = DenNameError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

#[cfg(test)]
mod tests;
