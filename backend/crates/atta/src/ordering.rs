use std::{cmp::Ordering, iter::Peekable, str::Chars};

/// U+03C2 GREEK SMALL LETTER FINAL SIGMA, which case folding maps to σ, as it does Σ.
const FINAL_SIGMA: char = 'ς';

/// One run of a name: a number, compared by value, or one other character, compared
/// case-insensitively.
#[derive(Debug, PartialEq, Eq)]
enum Run {
    /// A run of ASCII digits, with its leading zeros dropped.
    Number(String),
    /// One character that isn't an ASCII digit, lowercased, a final sigma as σ.
    Text(String),
}

impl Run {
    /// The character this run sorts by against a run of the other kind. A text run never starts
    /// with a digit and the digits are contiguous, so a number sorts against text the same way
    /// whatever its value, and the order stays total.
    fn lead(&self) -> char {
        match self {
            Run::Number(digits) => digits.chars().next().unwrap_or('0'),
            Run::Text(text) => text.chars().next().unwrap_or('\0'),
        }
    }
}

impl Ord for Run {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Run::Number(left), Run::Number(right)) => {
                left.len().cmp(&right.len()).then_with(|| left.cmp(right))
            }
            (Run::Text(left), Run::Text(right)) => left.cmp(right),
            _ => self.lead().cmp(&other.lead()),
        }
    }
}

impl PartialOrd for Run {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Splits a name into its runs, in order: each digit run whole, every other character alone.
struct Runs<'a> {
    characters: Peekable<Chars<'a>>,
}

impl Iterator for Runs<'_> {
    type Item = Run;

    fn next(&mut self) -> Option<Run> {
        let first = self.characters.next()?;
        let digit = first.is_ascii_digit();
        let mut run = String::from(first);
        while let Some(next) = self
            .characters
            .next_if(|next| digit && next.is_ascii_digit())
        {
            run.push(next);
        }
        if digit {
            let value = run.trim_start_matches('0');
            return Some(Run::Number(value.to_owned()));
        }
        let folded = run.to_lowercase().replace(FINAL_SIGMA, "σ");
        Some(Run::Text(folded))
    }
}

/// The runs of `name`, in order.
fn runs(name: &str) -> Runs<'_> {
    Runs {
        characters: name.chars().peekable(),
    }
}

/// The order names are listed in, as a file manager shows them: digit runs by their value,
/// other text without regard to case, then the names themselves, so the order is total.
pub(crate) fn name_order(left: &str, right: &str) -> Ordering {
    runs(left).cmp(runs(right)).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
