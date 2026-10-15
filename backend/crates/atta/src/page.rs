use std::{fmt, str::FromStr};

use crate::Entry;

/// The page-token format this build writes and reads.
const VERSION: u8 = 1;

/// How many hex digits a token holds: two for the version, sixteen for the offset.
const TOKEN_DIGITS: usize = 18;

/// An opaque position in one viewer's filtered listing: a version and an offset, never a name,
/// a hash or a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageToken {
    offset: usize,
}

impl PageToken {
    /// The token for the entry at `offset` in the filtered listing.
    pub(crate) fn at(offset: usize) -> Self {
        Self { offset }
    }

    /// The zero-based position it encodes in the filtered listing; it may lie past the end.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

/// Why a piece of text is not a [`PageToken`]: malformed, or from a format this build doesn't read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a page token is malformed or from another format")]
pub struct PageTokenError;

impl fmt::Display for PageToken {
    /// The version, then the offset, as lowercase hex.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{VERSION:02x}{:016x}", self.offset)
    }
}

impl FromStr for PageToken {
    type Err = PageTokenError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let lowercase_hex = text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        if text.len() != TOKEN_DIGITS || !lowercase_hex {
            return Err(PageTokenError);
        }
        let (version, offset) = text.split_at(2);
        let version = u8::from_str_radix(version, 16).map_err(|_| PageTokenError)?;
        if version != VERSION {
            return Err(PageTokenError);
        }
        let offset = u64::from_str_radix(offset, 16).map_err(|_| PageTokenError)?;
        let offset = usize::try_from(offset).map_err(|_| PageTokenError)?;
        Ok(Self { offset })
    }
}

/// One page of a directory's entries, after everything the viewer can't see is left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The entries on this page, in order.
    pub entries: Vec<Entry>,
    /// Where the next page starts; `None` on the last page.
    pub next: Option<PageToken>,
}

#[cfg(test)]
mod tests;
