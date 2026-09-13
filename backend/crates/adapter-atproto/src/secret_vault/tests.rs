use super::*;

const AAD: &[u8] = b"atproto_oauth.client_session\0did:plc:alice";
const PLAINTEXT: &[u8] = b"{\"refresh_token\":\"refresh-token\"}";

// Round-trip: seal then open under the same AAD yields the same plaintext.
#[test]
fn seal_open_round_trips() {
    let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
    let blob = vault.seal(AAD, PLAINTEXT).unwrap();
    assert_eq!(vault.open(AAD, &blob).unwrap().as_slice(), PLAINTEXT);
}

// The sealed blob must NOT contain the plaintext bytes — this is the whole
// point of encryption at rest.
#[test]
fn sealed_blob_is_not_plaintext() {
    let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
    let blob = vault.seal(AAD, PLAINTEXT).unwrap();
    assert_ne!(blob.as_slice(), PLAINTEXT);
    // The distinctive secret run does not survive into the ciphertext.
    let needle = b"refresh-token";
    assert!(
        !blob.windows(needle.len()).any(|w| w == needle),
        "plaintext secret leaked into the sealed blob"
    );
}

// A different root key cannot open the blob (AEAD tag fails).
#[test]
fn wrong_root_key_cannot_open() {
    let blob = SecretVault::from_bytes(&[7u8; 32])
        .unwrap()
        .seal(AAD, PLAINTEXT)
        .unwrap();
    assert!(
        SecretVault::from_bytes(&[8u8; 32])
            .unwrap()
            .open(AAD, &blob)
            .is_err()
    );
}

// The AAD is bound: a blob sealed under one row key cannot be opened under
// another, so sealed secrets cannot be swapped across rows.
#[test]
fn blob_cannot_be_opened_under_a_different_aad() {
    let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
    let blob = vault.seal(AAD, PLAINTEXT).unwrap();
    assert!(
        vault
            .open(b"atproto_oauth.client_session\0did:plc:mallory", &blob)
            .is_err()
    );
}

// A tampered blob fails to open (authenticity).
#[test]
fn tampered_blob_fails() {
    let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
    let mut blob = vault.seal(AAD, PLAINTEXT).unwrap();
    let last = blob.len() - 1;
    blob[last] ^= 0xFF;
    assert!(vault.open(AAD, &blob).is_err());
}

// A blob shorter than the nonce is rejected, not indexed out of bounds.
#[test]
fn short_blob_fails_closed() {
    let vault = SecretVault::from_bytes(&[7u8; 32]).unwrap();
    assert!(vault.open(AAD, &[0u8; NONCE_LEN - 1]).is_err());
}

// A wrong-length root key is rejected at construction.
#[test]
fn root_key_must_be_32_bytes() {
    assert!(SecretVault::from_bytes(&[0u8; 16]).is_err());
    assert!(SecretVault::from_bytes(&[0u8; 32]).is_ok());
}

// The vault's Debug must never reveal its bytes.
#[test]
fn vault_debug_is_redacted() {
    let vault = SecretVault::from_bytes(&[0xCD; 32]).unwrap();
    let shown = format!("{vault:?}");
    assert_eq!(shown, "SecretVault(<redacted>)");
    assert!(!shown.contains("cd") && !shown.contains("205"));
}
