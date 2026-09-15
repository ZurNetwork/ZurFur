use super::errors::StringBuilderViolation;

/// The explicit-rule string-validation builder. Rule methods take and return
/// `Self`, so a chain reads as the rules applied in order with no `?` until
/// [`build`](Self::build); once one fails, the rest short-circuit.
#[derive(Debug, Clone)]
pub struct StringBuilder(Result<String, StringBuilderViolation>);

impl StringBuilder {
    /// Start a chain over `raw`. No rule has run yet.
    pub fn new(raw: impl Into<String>) -> Self {
        Self(Ok(raw.into()))
    }

    /// Trim, then refuse empty: the two rules every free-text name shares.
    pub fn non_empty_from(raw: impl Into<String>) -> Self {
        StringBuilder::new(raw).trimmed().non_empty()
    }

    /// Trim leading/trailing whitespace. Cannot fail.
    pub fn trimmed(self) -> Self {
        Self(self.0.map(|s| {
            let trimmed = s.trim();
            if s.len() == trimmed.len() {
                s
            } else {
                trimmed.to_owned()
            }
        }))
    }

    /// Reject an empty value with [`StringBuilderViolation::Empty`]. Call after
    /// [`trimmed`](Self::trimmed) to reject whitespace-only input too.
    pub fn non_empty(self) -> Self {
        Self(self.0.and_then(|s| {
            if s.is_empty() {
                Err(StringBuilderViolation::Empty)
            } else {
                Ok(s)
            }
        }))
    }

    /// Reject a value longer than `max` trimmed `char`s with
    /// [`StringBuilderViolation::TooLong`].
    pub fn max_chars(self, max: usize) -> Self {
        Self(self.0.and_then(|s| {
            let len = s.chars().count();

            if len > max {
                Err(StringBuilderViolation::TooLong { max, len })
            } else {
                Ok(s)
            }
        }))
    }

    /// Reject any [`char::is_control`] character with
    /// [`StringBuilderViolation::ControlCharacter`].
    pub fn no_control(self) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(char::is_control) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// As [`no_control`](Self::no_control), except every character in
    /// `allowed` passes — the gate for multi-line free text.
    pub fn no_control_except(self, allowed: &[char]) -> Self {
        Self(self.0.and_then(|s| {
            if s.chars().any(|c| c.is_control() && !allowed.contains(&c)) {
                Err(StringBuilderViolation::ControlCharacter)
            } else {
                Ok(s)
            }
        }))
    }

    /// Finish the chain as a plain, rule-applied `String`, or the first rule
    /// violation recorded.
    pub fn build(self) -> Result<String, StringBuilderViolation> {
        self.0
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
