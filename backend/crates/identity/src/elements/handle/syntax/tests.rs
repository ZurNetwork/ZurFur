use super::*;

#[test]
fn split_labels_returns_the_labels_in_order() {
    let labels = split_labels("alice.bsky.social");
    let expected = Ok(vec!["alice", "bsky", "social"]);
    assert_eq!(labels, expected);
}

#[test]
fn split_labels_reports_the_first_fault_of_a_label() {
    // Over-long and hyphen-edged with a bad char: length is checked first.
    let long_bad_label = format!("-{}_", "a".repeat(63));
    let raw = format!("{long_bad_label}.com");
    assert_eq!(split_labels(&raw), Err(HandleError::SegmentTooLong(65)));
    // Hyphen-edged with a bad char: the hyphen edge is checked before charset.
    assert_eq!(split_labels("-a_.com"), Err(HandleError::HyphenEdge));
}

#[test]
fn check_top_level_uses_the_callers_list() {
    let labels = ["alice", "test"];
    assert_eq!(check_top_level(&labels, &[]), Ok(()));
    let expected_error = HandleError::ReservedTld("test".to_owned());
    assert_eq!(check_top_level(&labels, &["test"]), Err(expected_error));
}

#[test]
fn check_top_level_refuses_a_digit_leading_tld_before_the_list() {
    let labels = ["alice", "1test"];
    assert_eq!(
        check_top_level(&labels, &["1test"]),
        Err(HandleError::TldLeadingDigit)
    );
}

#[test]
fn check_top_level_refuses_no_labels() {
    assert_eq!(check_top_level(&[], &[]), Err(HandleError::TooFewSegments));
}
