use strum::VariantArray;

use super::*;
use crate::golden::{bytes_at, golden, hex_bytes, text_at};

#[test]
fn reproduces_rfc5869_test_case_1() {
    let ikm = [0x0b_u8; 22];
    let salt: Vec<u8> = (0x00..=0x0c).collect();
    let info: Vec<u8> = (0xf0..=0xf9).collect();
    let expected_okm = hex_bytes(
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865",
    );

    let mut okm = [0u8; 42];
    derive(&salt, &ikm, &info, &mut okm).expect("42 bytes expand");

    assert_eq!(okm.as_slice(), expected_okm.as_slice());
}

#[test]
fn purpose_keys_and_key_ids_match_the_golden_file() {
    let golden = golden();
    let root_bytes = bytes_at(&golden, "/root");
    let root = <[u8; KEY_BYTES]>::try_from(root_bytes.as_slice()).expect("a 32-byte root");

    for &purpose in Purpose::VARIANTS {
        let token: &'static str = purpose.into();
        let expected_key = bytes_at(&golden, &format!("/keys/{token}/key"));
        let expected_key_id = text_at(&golden, &format!("/keys/{token}/key_id"));

        let key = purpose_key(&root, purpose);
        let derived_key_id = key_id(&root, purpose);

        assert_eq!(key.as_slice(), expected_key.as_slice(), "{token}'s key");
        assert_eq!(derived_key_id.as_ref(), expected_key_id, "{token}'s key id");
    }
}

#[test]
fn purpose_keys_are_pairwise_distinct_and_not_the_root() {
    let root: [u8; KEY_BYTES] = std::array::from_fn(|index| u8::try_from(index).unwrap_or(0));
    let keys: Vec<Zeroizing<[u8; KEY_BYTES]>> = Purpose::VARIANTS
        .iter()
        .copied()
        .map(|purpose| purpose_key(&root, purpose))
        .collect();
    let key_ids: Vec<KeyId> = Purpose::VARIANTS
        .iter()
        .copied()
        .map(|purpose| key_id(&root, purpose))
        .collect();

    for (index, key) in keys.iter().enumerate() {
        assert_ne!(**key, root, "a purpose key is not the root");
        for other in &keys[index + 1..] {
            assert_ne!(**key, **other, "two purposes share a key");
        }
    }
    for (index, key_id) in key_ids.iter().enumerate() {
        for other in &key_ids[index + 1..] {
            assert_ne!(key_id, other, "two purposes share a key id");
        }
    }
}

#[test]
fn key_ids_name_their_purpose_in_lowercase_hex() {
    let root = [7u8; KEY_BYTES];

    for &purpose in Purpose::VARIANTS {
        let derived_key_id = key_id(&root, purpose);
        let token: &'static str = purpose.into();
        let prefix = format!("hkdf:{token}:");
        let fingerprint = derived_key_id
            .as_ref()
            .strip_prefix(prefix.as_str())
            .expect("the key id starts with its scheme and purpose");

        assert_eq!(derived_key_id.purpose(), Some(purpose));
        assert_eq!(fingerprint.len(), 2 * FINGERPRINT_BYTES);
        assert!(
            fingerprint
                .bytes()
                .all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f'))
        );
    }
}
