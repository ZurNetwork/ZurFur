use super::*;

fn keys() -> AccountKeys {
    AccountKeys {
        cold_recovery: SecretKey::new(vec![0xAA; 32]),
        operational: SecretKey::new(vec![0xBB; 32]),
        signing: SecretKey::new(vec![0xCC; 32]),
    }
}

const DID: &str = "did:plc:alice";

// Round-trip: wrap then unwrap under the same DID yields the same keys.
#[test]
fn wrap_unwrap_round_trips() {
    let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
    let blob = root.wrap(DID, &keys()).unwrap();
    assert_eq!(root.unwrap(DID, &blob).unwrap(), keys());
}

// The sealed blob must NOT contain the plaintext key bytes — this is the whole
// point of encryption at rest.
#[test]
fn wrapped_blob_is_not_plaintext() {
    let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
    let blob = root.wrap(DID, &keys()).unwrap();
    // None of the three 32-byte plaintext runs appears in the ciphertext.
    for byte in [0xAAu8, 0xBB, 0xCC] {
        let run = vec![byte; 32];
        assert!(
            !blob.windows(32).any(|w| w == run.as_slice()),
            "plaintext key bytes ({byte:#x}) leaked into the wrapped blob"
        );
    }
}

// A different root key cannot open the blob (AEAD tag fails).
#[test]
fn wrong_root_key_cannot_unwrap() {
    let blob = RootKey::from_bytes(&[7u8; 32])
        .unwrap()
        .wrap(DID, &keys())
        .unwrap();
    assert!(
        RootKey::from_bytes(&[8u8; 32])
            .unwrap()
            .unwrap(DID, &blob)
            .is_err()
    );
}

// The DID is bound as AEAD associated data: a blob sealed for one DID cannot be
// opened under another, so custody rows cannot be swapped across accounts.
#[test]
fn blob_cannot_be_opened_under_a_different_did() {
    let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
    let blob = root.wrap(DID, &keys()).unwrap();
    assert!(root.unwrap("did:plc:mallory", &blob).is_err());
}

// A tampered blob fails to open (authenticity).
#[test]
fn tampered_blob_fails() {
    let root = RootKey::from_bytes(&[7u8; 32]).unwrap();
    let mut blob = root.wrap(DID, &keys()).unwrap();
    let last = blob.len() - 1;
    blob[last] ^= 0xFF;
    assert!(root.unwrap(DID, &blob).is_err());
}

// A wrong-length root key is rejected at construction.
#[test]
fn root_key_must_be_32_bytes() {
    assert!(RootKey::from_bytes(&[0u8; 16]).is_err());
    assert!(RootKey::from_bytes(&[0u8; 32]).is_ok());
}

// The root key's Debug must never reveal its bytes.
#[test]
fn root_key_debug_is_redacted() {
    let root = RootKey::from_bytes(&[0xCD; 32]).unwrap();
    let shown = format!("{root:?}");
    assert_eq!(shown, "RootKey(<redacted>)");
    assert!(!shown.contains("cd") && !shown.contains("205"));
}
