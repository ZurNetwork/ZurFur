use std::str::FromStr;

/// An opaque position in one viewer's listing of a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display)]
pub struct PageToken(atta::PageToken);

impl PageToken {
    /// The engine's token this one carries; inherent, because a public `Into` would put the
    /// engine's type in this crate's API.
    pub(crate) fn into_inner(self) -> atta::PageToken {
        self.0
    }

    /// Wraps a token the engine answered with; inherent, because a public `From` would put
    /// the engine's type in this crate's API.
    pub(crate) fn new(token: atta::PageToken) -> Self {
        Self(token)
    }
}

/// Why a piece of text is not a [`PageToken`]: malformed, or from a format this build doesn't read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a page token is malformed or from another format")]
pub struct PageTokenError;

impl FromStr for PageToken {
    type Err = PageTokenError;

    /// The engine's own parse, its refusal mapped to this crate's error so a driver never
    /// names the engine.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let token = atta::PageToken::from_str(text).map_err(|_| PageTokenError)?;
        Ok(Self(token))
    }
}

#[cfg(test)]
mod tests;
