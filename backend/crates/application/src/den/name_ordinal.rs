/// The number that tells a node apart from the earlier nodes of the same name its viewer sees
/// in one folder, shown as "name (n)"; never below 2, and never part of the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display)]
pub struct NameOrdinal(atta::NameOrdinal);

impl NameOrdinal {
    /// Wraps a number the engine answered with; inherent, because a public `From` would put
    /// the engine's type in this crate's API.
    pub(crate) fn new(ordinal: atta::NameOrdinal) -> Self {
        Self(ordinal)
    }
}

/// Why a number is not a [`NameOrdinal`]: it is below 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a name ordinal is 2 or more")]
pub struct NameOrdinalError;

impl TryFrom<u32> for NameOrdinal {
    type Error = NameOrdinalError;

    /// Any number from 2 up, by the engine's own rule.
    fn try_from(number: u32) -> Result<Self, Self::Error> {
        let ordinal = atta::NameOrdinal::try_from(number).map_err(|_| NameOrdinalError)?;
        Ok(Self(ordinal))
    }
}

impl From<NameOrdinal> for u32 {
    /// The number itself.
    fn from(ordinal: NameOrdinal) -> Self {
        u32::from(ordinal.0)
    }
}

#[cfg(test)]
mod tests;
