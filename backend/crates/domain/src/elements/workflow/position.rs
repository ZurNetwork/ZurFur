use std::str::FromStr;

use super::errors::PositionError;

/// Digits of a [`Position`], in byte order — so `Ord` on the string is `Ord` on the key.
const POSITION_DIGITS: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const POSITION_BASE: usize = POSITION_DIGITS.len();

/// A column's place among its siblings: a base-62 fractional key compared
/// bytewise. Inserting mints a key *between* two neighbours; nothing is ever
/// renumbered. A store must order it bytewise too (`text COLLATE "C"`).
///
/// Invariant: non-empty, alphabet digits only, never ends in `0` — that is
/// what guarantees [`between`](Position::between) always has room.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position(String);

impl Position {
    /// A key strictly between `lo` and `hi`; `None` stands for the start
    /// (`lo`) or the end (`hi`) of the sequence.
    ///
    /// # Panics
    /// If `lo` does not sort before `hi` — a caller bug, since neighbours come
    /// from an already-ordered sequence.
    pub fn between(lo: Option<&Position>, hi: Option<&Position>) -> Position {
        let lo = lo.map_or(&[][..], |p| p.0.as_bytes());
        let hi = hi.map(|p| p.0.as_bytes());
        assert!(hi.is_none_or(|hi| lo < hi), "lo must sort before hi");

        let mut out = Vec::new();
        midpoint(lo, hi, &mut out);
        Position(String::from_utf8(out).expect("the alphabet is ASCII"))
    }
}

/// Appends a key strictly between `lo` and `hi` to `out`; `hi == None` is +∞.
fn midpoint(mut lo: &[u8], mut hi: Option<&[u8]>, out: &mut Vec<u8>) {
    loop {
        if let Some(h) = hi {
            let shared = shared_prefix(lo, h);
            if shared > 0 {
                out.extend_from_slice(&h[..shared]);
                lo = lo.get(shared..).unwrap_or(&[]);
                hi = Some(&h[shared..]);
                continue;
            }
        }

        let digit_lo = lo.first().map_or(0, |d| digit_index(*d));
        let digit_hi = hi
            .and_then(|h| h.first())
            .map_or(POSITION_BASE, |d| digit_index(*d));

        if digit_hi - digit_lo > 1 {
            out.push(POSITION_DIGITS[(digit_lo + digit_hi).div_ceil(2)]);
            return;
        }

        // The digits are adjacent: no room at this place.
        match hi {
            // `hi` continues past this digit, so its first digit alone sits below it.
            Some(h) if h.len() > 1 => {
                out.push(h[0]);
                return;
            }
            // Otherwise step into `lo`'s next place with nothing above it.
            _ => {
                out.push(POSITION_DIGITS[digit_lo]);
                lo = lo.get(1..).unwrap_or(&[]);
                hi = None;
            }
        }
    }
}

/// Length of the prefix `lo` and `hi` share; `lo` is read as zero-padded.
fn shared_prefix(lo: &[u8], hi: &[u8]) -> usize {
    hi.iter()
        .enumerate()
        .take_while(|(i, h)| lo.get(*i).copied().unwrap_or(POSITION_DIGITS[0]) == **h)
        .count()
}

fn digit_index(digit: u8) -> usize {
    POSITION_DIGITS
        .iter()
        .position(|d| *d == digit)
        .expect("a Position holds only alphabet digits")
}

impl FromStr for Position {
    type Err = PositionError;

    /// Validate a stored key against the [`Position`] invariant.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(PositionError::Empty);
        }
        if let Some(c) = s.chars().find(|c| !c.is_ascii_alphanumeric()) {
            return Err(PositionError::InvalidDigit(c));
        }
        if s.ends_with('0') {
            return Err(PositionError::TrailingZero);
        }
        Ok(Self(s.to_owned()))
    }
}

impl AsRef<str> for Position {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
