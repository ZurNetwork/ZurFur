use super::HandleError;
use super::reserved::{HANDLE_MAX_LEN, LABEL_MAX_LEN};

/// Splits an already-normalized handle into its labels, checking the overall
/// length, that there are at least two labels, and each label's length, hyphen
/// edges and `[a-z0-9-]` charset.
pub(super) fn split_labels(normalized: &str) -> Result<Vec<&str>, HandleError> {
    if normalized.is_empty() {
        return Err(HandleError::Empty);
    }
    let len = normalized.chars().count();
    if len > HANDLE_MAX_LEN {
        return Err(HandleError::TooLong(len));
    }

    let labels: Vec<&str> = normalized.split('.').collect();
    if labels.len() < 2 {
        return Err(HandleError::TooFewSegments);
    }
    if labels.iter().any(|label| label.is_empty()) {
        return Err(HandleError::EmptySegment);
    }

    for label in &labels {
        check_label(label)?;
    }
    Ok(labels)
}

/// Checks one label's length, hyphen edges and charset, in that order, so a
/// malformed label reports its first fault.
fn check_label(label: &str) -> Result<(), HandleError> {
    let label_len = label.chars().count();
    if label_len > LABEL_MAX_LEN {
        return Err(HandleError::SegmentTooLong(label_len));
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err(HandleError::HyphenEdge);
    }
    let invalid_char = label
        .chars()
        .find(|&c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
    if let Some(bad) = invalid_char {
        return Err(HandleError::InvalidChar(bad));
    }
    Ok(())
}

/// Checks the rightmost label of `labels` (as [`split_labels`] returned them):
/// it must not start with a digit, nor be one of the caller's `reserved_tlds`.
pub(super) fn check_top_level(labels: &[&str], reserved_tlds: &[&str]) -> Result<(), HandleError> {
    let Some(&tld) = labels.last() else {
        return Err(HandleError::TooFewSegments);
    };
    if tld.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(HandleError::TldLeadingDigit);
    }
    if reserved_tlds.contains(&tld) {
        return Err(HandleError::ReservedTld(tld.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
