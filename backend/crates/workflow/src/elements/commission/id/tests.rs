use super::*;

// A minted commission key is a UUIDv7, so it sorts by creation time.
#[test]
fn a_minted_key_is_a_v7_uuid() {
    let minted = CommissionId::mint();
    let version = minted.as_ref().get_version_num();
    assert_eq!(version, 7);
}

// Two mints never answer the same key.
#[test]
fn two_mints_differ() {
    let first = CommissionId::mint();
    let second = CommissionId::mint();
    assert_ne!(first, second);
}
