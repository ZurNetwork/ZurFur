use super::*;

#[test]
fn a_root_that_is_not_32_bytes_is_refused_by_its_length_only() {
    let short_root = [0x7Au8; 16];

    let refusal = did_custody_sealer(&short_root, &Environment::DEV, false)
        .expect_err("a 16-byte root is refused");

    let message = format!("{refusal:#}");
    assert_eq!(
        message,
        "the custody root key must be exactly 32 bytes, got 16"
    );
}

#[test]
fn the_guard_refuses_before_the_length_is_read() {
    let short_root = [0x7Au8; 16];

    let refusal = did_custody_sealer(&short_root, &Environment::PROD, false)
        .expect_err("production refuses a config root");

    let message = format!("{refusal:#}");
    assert!(message.starts_with("refusing to boot in PROD"), "{message}");
}

#[test]
fn a_32_byte_dev_root_gives_the_did_custody_handle() {
    let root = [0x7Au8; 32];

    let sealer = did_custody_sealer(&root, &Environment::DEV, false).expect("a dev root");

    let shown = format!("{sealer:?}");
    assert_eq!(shown, "ScopedSealer { purpose: DidCustody }");
}
