use std::error::Error;

use domain::elements::commission::{FileNameError, MarkupError};

use super::CommissionError;
use crate::{common_error::CommonError, den::DenError};

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

#[test]
fn den_s_one_not_found_becomes_the_answer_the_read_names() {
    let not_a_member = super::den_refusal(DenError::NotFound, CommissionError::NotAMember);
    let file_not_found = super::den_refusal(DenError::NotFound, CommissionError::FileNotFound);

    assert!(matches!(not_a_member, CommissionError::NotAMember));
    assert!(matches!(file_not_found, CommissionError::FileNotFound));
}

#[test]
fn a_den_store_failure_stays_a_store_failure() {
    let store_down = DenError::Store(anyhow::anyhow!("the commission store is down"));

    let answer = super::den_refusal(store_down, CommissionError::NotAMember);

    assert!(matches!(
        answer,
        CommissionError::CommonError(CommonError::Infrastructure(_))
    ));
}
