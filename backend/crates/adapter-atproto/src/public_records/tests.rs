use super::is_record_not_found;

// Finding 3: only the exact `RecordNotFound` code (bare, or in jacquard's
// `"RecordNotFound: <message>"` typed render) is a missing *record*; every other
// `*NotFound` is a real repo/account/identity failure that must keep its status.
#[test]
fn record_not_found_is_the_only_missing_record_signal() {
    // The record-absent code — bare, and jacquard's typed render with the PDS
    // "could not locate record" message appended — both classify as NotFound.
    assert!(is_record_not_found("RecordNotFound"));
    assert!(is_record_not_found(
        "RecordNotFound: Could not locate record: at://did:plc:x/app.zurfur.feed.post/1"
    ));

    // Repo-, account- and identity-level failures merely CONTAIN "NotFound"; they
    // are genuine errors, never an absent record, so must NOT be flattened.
    assert!(!is_record_not_found("RepoNotFound"));
    assert!(!is_record_not_found("AccountNotFound"));
    assert!(!is_record_not_found("RepoDeactivated"));
    assert!(!is_record_not_found("InvalidRequest"));
    // A bare "NotFound" is not the record code either — the old substring bug.
    assert!(!is_record_not_found("NotFound"));
}
