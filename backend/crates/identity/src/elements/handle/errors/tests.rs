use super::*;

// ---- Error quality -----------------------------------------------------

#[test]
fn every_error_variant_renders_a_message() {
    let variants = [
        HandleError::Empty,
        HandleError::TooLong(300),
        HandleError::TooFewSegments,
        HandleError::EmptySegment,
        HandleError::SegmentTooLong(64),
        HandleError::InvalidChar('_'),
        HandleError::HyphenEdge,
        HandleError::TldLeadingDigit,
        HandleError::ReservedTld("local".into()),
        HandleError::PunycodeLabel,
        HandleError::ReservedLabel("api".into()),
        HandleError::NamespaceApex,
    ];
    for v in variants {
        assert!(!v.to_string().is_empty(), "{v:?} rendered an empty message");
    }
}

#[test]
fn domain_error_renders_a_message() {
    assert!(!HandleDomainError::Empty.to_string().is_empty());
    let not_a_hostname = HandleDomainError::NotAHostname(HandleError::InvalidChar(':'));
    assert!(!not_a_hostname.to_string().is_empty());
}
