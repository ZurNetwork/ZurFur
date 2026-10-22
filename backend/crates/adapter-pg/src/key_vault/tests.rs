use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::elements::account_keys::AccountKeys;
use test_support::custody::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_BLOB_STARTING_0X01_HEX, LEGACY_ALICE_DID,
    LEGACY_BOB_BLOB_HEX, LEGACY_BOB_DID, LEGACY_ROOT, hex_bytes, legacy_keys,
};

use super::*;

/// The 0.1 root the fixtures are sealed under.
fn legacy_root() -> LegacyRootKey {
    LegacyRootKey::new(Zeroizing::new(LEGACY_ROOT))
}

/// Seals `plaintext` for `did` in the 0.1 format under `root`, with a fixed nonce.
fn seal_0_1(root: &[u8; 32], did: &str, plaintext: &[u8]) -> Vec<u8> {
    let nonce = [3u8; NONCE_LEN];
    let payload = Payload {
        msg: plaintext,
        aad: did.as_bytes(),
    };
    let ciphertext = XChaCha20Poly1305::new(root.into())
        .encrypt(XNonce::from_slice(&nonce), payload)
        .expect("seal a 0.1 test blob");
    [nonce.as_slice(), ciphertext.as_slice()].concat()
}

#[test]
fn legacy_fixtures_open_under_the_0_1_root() {
    let root = legacy_root();
    let alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);
    let bob_blob = hex_bytes(LEGACY_BOB_BLOB_HEX);
    let alice_blob_starting_0x01 = hex_bytes(LEGACY_ALICE_BLOB_STARTING_0X01_HEX);
    let expected_keys = legacy_keys();

    let alice_keys = root.open(LEGACY_ALICE_DID, &alice_blob).unwrap();
    assert_eq!(AccountKeys::from(&alice_keys), expected_keys);
    let bob_keys = root.open(LEGACY_BOB_DID, &bob_blob).unwrap();
    assert_eq!(AccountKeys::from(&bob_keys), expected_keys);
    let alice_keys_from_0x01 = root
        .open(LEGACY_ALICE_DID, &alice_blob_starting_0x01)
        .unwrap();
    assert_eq!(AccountKeys::from(&alice_keys_from_0x01), expected_keys);

    let expected_first_byte = 0x01u8;
    assert_eq!(alice_blob_starting_0x01[0], expected_first_byte);

    let alice_blob_as_bob = root.open(LEGACY_BOB_DID, &alice_blob);
    assert_eq!(
        alice_blob_as_bob.err(),
        Some(LegacyOpenError::Unopenable),
        "alice's blob opened under bob's DID — the DID is not bound"
    );
}

#[test]
fn a_wrong_root_cannot_open() {
    let wrong_root = LegacyRootKey::new(Zeroizing::new([8u8; 32]));
    let alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);

    let opened = wrong_root.open(LEGACY_ALICE_DID, &alice_blob);

    assert_eq!(opened.err(), Some(LegacyOpenError::Unopenable));
}

#[test]
fn a_tampered_blob_cannot_open() {
    let mut alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);
    let last = alice_blob.len() - 1;
    alice_blob[last] ^= 0xFF;

    let opened = legacy_root().open(LEGACY_ALICE_DID, &alice_blob);

    assert_eq!(opened.err(), Some(LegacyOpenError::Unopenable));
}

#[test]
fn a_blob_shorter_than_its_nonce_cannot_open() {
    let short_blob = [7u8; NONCE_LEN - 1];

    let opened = legacy_root().open(LEGACY_ALICE_DID, &short_blob);

    assert_eq!(opened.err(), Some(LegacyOpenError::Unopenable));
}

#[test]
fn a_plaintext_that_is_not_three_keys_is_refused() {
    let two_keys = [0xAAu8; 64];
    let blob = seal_0_1(&LEGACY_ROOT, LEGACY_ALICE_DID, &two_keys);

    let opened = legacy_root().open(LEGACY_ALICE_DID, &blob);

    assert_eq!(opened.err(), Some(LegacyOpenError::NotThreeKeys));
}

#[test]
fn a_blob_sealed_by_the_test_helper_opens() {
    let three_keys = [0x11u8; 96];
    let blob = seal_0_1(&LEGACY_ROOT, LEGACY_ALICE_DID, &three_keys);

    let opened = legacy_root().open(LEGACY_ALICE_DID, &blob).unwrap();

    assert_eq!(opened.expose(), three_keys.as_slice());
}

#[test]
fn the_legacy_root_debug_prints_nothing_of_it() {
    let root_bytes = [0xCDu8; 32];
    let root = LegacyRootKey::new(Zeroizing::new(root_bytes));

    let shown = format!("{root:?}");

    assert_eq!(shown, "LegacyRootKey(<redacted>)");
    assert!(!shown.contains("cd") && !shown.contains("CD") && !shown.contains("205"));
}
