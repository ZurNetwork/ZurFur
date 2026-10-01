use std::error::Error;

use domain::elements::commission::{FileNameError, MarkupError};

use super::CommissionError;

#[test]
fn an_invalid_file_name_keeps_its_cause_on_source() {
    let error = CommissionError::InvalidFileName(FileNameError::Empty);

    assert!(error.source().is_some());
}

#[test]
fn an_invalid_markup_keeps_its_cause_on_source() {
    let cause = MarkupError::CoordinateOutOfRange("x", 2.0);
    let error = CommissionError::InvalidMarkup(cause);

    assert!(error.source().is_some());
}
