use std::str::FromStr;

use super::percent::PercentDecoded;

/// The most raw pieces decoded: one past the Den's 16-piece cap, so a longer path still
/// reaches the application's not-found without decoding the rest.
const MAX_PIECES: usize = 17;

/// A raw Den path split on `/` first, then each piece percent-decoded exactly once, so a
/// decoded `/` stays inside its piece. An empty path is no pieces; past 17 pieces the rest is
/// dropped undecoded.
#[derive(Debug, Clone, Default, PartialEq, Eq, derive_more::Into)]
pub(super) struct DenPieces(Vec<String>);

/// Why a raw path has no pieces: one doesn't decode to UTF-8. Carries no part of the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a den path piece is not percent-encoded UTF-8")]
pub(super) struct NotDenPieces;

impl FromStr for DenPieces {
    type Err = NotDenPieces;

    /// Splits the raw path, still percent-encoded, keeps at most 17 pieces, then decodes each
    /// once; empty pieces are kept for the application to refuse.
    fn from_str(raw_path: &str) -> Result<Self, Self::Err> {
        if raw_path.is_empty() {
            return Ok(Self::default());
        }
        let pieces = raw_path
            .split('/')
            .take(MAX_PIECES)
            .map(|raw_piece| {
                raw_piece
                    .parse::<PercentDecoded>()
                    .map(String::from)
                    .map_err(|_| NotDenPieces)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self(pieces))
    }
}

#[cfg(test)]
mod tests;
