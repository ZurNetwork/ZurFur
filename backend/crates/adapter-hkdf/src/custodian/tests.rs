use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::elements::custody::ENVELOPE_VERSION;
use serde_json::Value;
use strum::VariantArray;

use super::*;
use crate::derivation::{
    FINGERPRINT_BYTES, KEY_ID_INFO_PREFIX, KEY_INFO_PREFIX, SALT, info_label, purpose_key,
};
use crate::envelope::{AEAD_TAG_BYTES, NONCE_BYTES};
use crate::golden::{
    array_at, bytes_at, golden, hex_bytes, hex_text, mac_message, number_at, subject, test_root,
    text_at,
};
use crate::mac::{MAC_TAG_BYTES, mac_label};

/// A real 0.1 blob whose first byte is `0x01`: `nonce ‖ ciphertext`, no version, no key id.
const LEGACY_ALICE_BLOB_STARTING_0X01_HEX: &str = "017c34555bb478e165076720364596499e1ab8c224c4a65ec7907f78b40ec4dc48043e5ed67aa6fad1053c4eba0306264a360d7492f45c95252c44fb91401ee504803dc33c351d65a2948edaec168ac913e8aacd09cfdc1ec3e86fcf28ecd37771cd31fe3f52054a6daa4dc49049dc6fdcaf0e61391320a9d4fab3551254e1ae2068ecdc45ba2def";

/// A test keyring from the golden test root, holding `purposes`.
fn test_keyring(purposes: &[Purpose]) -> HkdfKeyCustodian {
    HkdfKeyCustodian::derive(test_root(), purposes)
}

/// A test keyring from the golden test root, holding every purpose.
fn full_keyring() -> HkdfKeyCustodian {
    test_keyring(Purpose::VARIANTS)
}

/// A test keyring from another root, holding every purpose.
fn other_root_keyring() -> HkdfKeyCustodian {
    let other_root = Zeroizing::new([0xa5; KEY_BYTES]);
    HkdfKeyCustodian::derive(other_root, Purpose::VARIANTS)
}

/// A test keyring holding exactly `entries`, built around the constructor.
fn keyring_holding(entries: &[(SealPurpose, &str, [u8; KEY_BYTES])]) -> HkdfKeyCustodian {
    let seal_keys = entries
        .iter()
        .map(|&(purpose, key_id_text, key)| SealKey {
            purpose,
            key_id: KeyId::try_from(key_id_text).expect("a test key id"),
            key: Zeroizing::new(key),
        })
        .collect();
    HkdfKeyCustodian {
        seal_keys,
        den_mac: None,
    }
}

/// A subject made of `parts`.
fn subject_of(parts: &[&[u8]]) -> CustodySubject {
    CustodySubject::try_from(parts).expect("a short subject encodes")
}

/// Where a sealed envelope's nonce starts: after the version, the length and the key id.
fn nonce_start(sealed: &SealedEnvelope) -> usize {
    2 + usize::from(sealed.as_ref()[1])
}

/// `sealed` with the byte at `index` flipped.
fn flipped(sealed: &SealedEnvelope, index: usize) -> SealedEnvelope {
    let mut bytes = sealed.as_ref().to_vec();
    bytes[index] ^= 0x01;
    SealedEnvelope::from(bytes)
}

/// A secret sealed under the full keyring's DID custody key for Alice's account keys.
async fn alice_envelope(custodian: &HkdfKeyCustodian) -> SealedEnvelope {
    custodian
        .seal(
            SealPurpose::DidCustody,
            &subject("account_keys"),
            b"alice's custody keys",
        )
        .await
        .expect("the keyring seals")
}

#[test]
fn constants_match_the_golden_file() {
    let golden = golden();

    assert_eq!(
        vec![ENVELOPE_VERSION],
        bytes_at(&golden, "/constants/version")
    );
    assert_eq!(SALT, bytes_at(&golden, "/constants/salt/hex"));
    assert_eq!(KEY_BYTES, number_at(&golden, "/constants/key_length"));
    assert_eq!(
        FINGERPRINT_BYTES,
        number_at(&golden, "/constants/key_id_fingerprint_length")
    );
    assert_eq!(NONCE_BYTES, number_at(&golden, "/constants/nonce_length"));
    assert_eq!(
        AEAD_TAG_BYTES,
        number_at(&golden, "/constants/aead_tag_length")
    );
    assert_eq!(
        MAC_TAG_BYTES,
        number_at(&golden, "/constants/mac_tag_length")
    );
    for &purpose in Purpose::VARIANTS {
        let token: &'static str = purpose.into();
        let info = bytes_at(&golden, &format!("/constants/purposes/{token}/info/hex"));
        let key_id_info = bytes_at(
            &golden,
            &format!("/constants/purposes/{token}/key_id_info/hex"),
        );

        assert_eq!(info_label(KEY_INFO_PREFIX, purpose), info, "{token}");
        assert_eq!(
            info_label(KEY_ID_INFO_PREFIX, purpose),
            key_id_info,
            "{token}"
        );
    }
    for &mac_use in MacUse::VARIANTS {
        let token: &'static str = mac_use.into();
        let label = bytes_at(&golden, &format!("/constants/mac_uses/{token}/label/hex"));

        assert_eq!(mac_label(mac_use), label, "{token}");
    }
}

#[test]
fn subject_and_mac_message_encodings_match_the_golden_file() {
    let golden = golden();
    let subjects = golden
        .pointer("/subjects")
        .and_then(Value::as_object)
        .expect("the golden file has subjects");

    for name in ["account_keys", "client_session", "auth_request"] {
        let expected_encoding = bytes_at(&subjects[name], "/encoding");

        assert_eq!(
            hex_text(subject(name).as_ref()),
            hex_text(&expected_encoding),
            "{name}"
        );
    }
    for (index, message) in array_at(&golden, "/mac_messages").iter().enumerate() {
        let expected_encoding = bytes_at(message, "/encoding");

        assert_eq!(
            hex_text(mac_message(index).as_ref()),
            hex_text(&expected_encoding)
        );
    }
}

#[tokio::test]
async fn golden_envelopes_open() {
    let golden = golden();
    let custodian = full_keyring();
    let envelopes = array_at(&golden, "/envelopes");
    let seal_purposes: Vec<&str> = envelopes
        .iter()
        .map(|envelope| text_at(envelope, "/purpose"))
        .collect();
    for &seal_purpose in SealPurpose::VARIANTS {
        let token: &'static str = seal_purpose.into();
        assert!(
            seal_purposes.contains(&token),
            "{token} has no golden envelope"
        );
    }

    for envelope in envelopes {
        let purpose: SealPurpose = text_at(envelope, "/purpose").parse().expect("a purpose");
        let sealed_bytes = [
            bytes_at(envelope, "/header"),
            bytes_at(envelope, "/ciphertext"),
        ]
        .concat();
        let expected_plaintext = bytes_at(envelope, "/plaintext");

        let opened = custodian
            .open(
                purpose,
                &subject(text_at(envelope, "/subject")),
                &SealedEnvelope::from(sealed_bytes),
            )
            .await
            .expect("a golden envelope opens");

        assert_eq!(opened.expose(), expected_plaintext.as_slice());
    }
}

#[tokio::test]
async fn a_sealed_secret_opens_for_its_purpose_and_subject() {
    let custodian = full_keyring();
    let subject = subject("client_session");

    let sealed = custodian
        .seal(SealPurpose::SigninSecrets, &subject, b"session secret")
        .await
        .expect("the keyring seals");
    let opened = custodian
        .open(SealPurpose::SigninSecrets, &subject, &sealed)
        .await
        .expect("the envelope opens");

    assert_eq!(opened.expose(), b"session secret");
}

#[tokio::test]
async fn purpose_is_bound_even_when_keys_match() {
    let shared_key_id = "hkdf:did_custody:0011223344556677";
    let custodian = keyring_holding(&[
        (SealPurpose::DidCustody, shared_key_id, [0x42; KEY_BYTES]),
        (SealPurpose::SigninSecrets, shared_key_id, [0x42; KEY_BYTES]),
    ]);
    let subject = subject("account_keys");
    let sealed = custodian
        .seal(SealPurpose::DidCustody, &subject, b"custody keys")
        .await
        .expect("the keyring seals");

    let opened = custodian
        .open(SealPurpose::SigninSecrets, &subject, &sealed)
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::Unauthentic));
}

#[tokio::test]
async fn sealed_for_alice_wont_open_for_mallory() {
    let custodian = full_keyring();
    let sealed = alice_envelope(&custodian).await;
    let mallory: [&[u8]; 2] = [b"account_keys", b"did:plc:mallory"];

    let opened = custodian
        .open(SealPurpose::DidCustody, &subject_of(&mallory), &sealed)
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::Unauthentic));
}

#[tokio::test]
async fn a_blob_moved_to_another_row_wont_open() {
    let custodian = full_keyring();
    let first_row: [&[u8]; 3] = [b"client_session", b"did:plc:alice", b"session-1"];
    let second_row: [&[u8]; 3] = [b"client_session", b"did:plc:alice", b"session-2"];
    let sealed = custodian
        .seal(
            SealPurpose::SigninSecrets,
            &subject_of(&first_row),
            b"session one",
        )
        .await
        .expect("the keyring seals");

    let opened = custodian
        .open(
            SealPurpose::SigninSecrets,
            &subject_of(&second_row),
            &sealed,
        )
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::Unauthentic));
}

#[tokio::test]
async fn swapped_key_id_fails_even_with_the_same_key_bytes() {
    let first_key_id = "hkdf:did_custody:0000000000000001";
    let second_key_id = "hkdf:did_custody:0000000000000002";
    let sealer = keyring_holding(&[(SealPurpose::DidCustody, first_key_id, [0x42; KEY_BYTES])]);
    let opener = keyring_holding(&[(SealPurpose::DidCustody, second_key_id, [0x42; KEY_BYTES])]);
    let subject = subject("account_keys");
    let sealed = sealer
        .seal(SealPurpose::DidCustody, &subject, b"custody keys")
        .await
        .expect("the keyring seals");
    let mut swapped_bytes = sealed.as_ref().to_vec();
    swapped_bytes[2..2 + second_key_id.len()].copy_from_slice(second_key_id.as_bytes());

    let opened = opener
        .open(
            SealPurpose::DidCustody,
            &subject,
            &SealedEnvelope::from(swapped_bytes),
        )
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::Unauthentic));
}

#[tokio::test]
async fn another_purposes_key_id_is_unknown_here() {
    let custodian = full_keyring();
    let subject = subject("account_keys");
    let sealed = custodian
        .seal(SealPurpose::SigninSecrets, &subject, b"a sign-in secret")
        .await
        .expect("the keyring seals");
    let signin_key_id = custodian
        .seal_key(SealPurpose::SigninSecrets)
        .expect("held")
        .key_id
        .clone();
    let unknown_key_id = CustodyError::UnknownKeyId {
        key_id: signin_key_id,
    };

    let opened = custodian
        .open(SealPurpose::DidCustody, &subject, &sealed)
        .await
        .err();

    assert_eq!(opened, Some(unknown_key_id));
}

#[tokio::test]
async fn unknown_key_id_is_refused() {
    let other = other_root_keyring();
    let sealed = alice_envelope(&other).await;
    let other_key_id = other
        .seal_key(SealPurpose::DidCustody)
        .expect("held")
        .key_id
        .clone();
    let unknown_key_id = CustodyError::UnknownKeyId {
        key_id: other_key_id,
    };

    let opened = full_keyring()
        .open(SealPurpose::DidCustody, &subject("account_keys"), &sealed)
        .await
        .err();

    assert_eq!(opened, Some(unknown_key_id));
}

#[tokio::test]
async fn tampered_nonce_fails() {
    let custodian = full_keyring();
    let sealed = alice_envelope(&custodian).await;
    let start = nonce_start(&sealed);

    for index in start..start + NONCE_BYTES {
        let opened = custodian
            .open(
                SealPurpose::DidCustody,
                &subject("account_keys"),
                &flipped(&sealed, index),
            )
            .await
            .err();

        assert_eq!(
            opened,
            Some(CustodyError::Unauthentic),
            "nonce byte {index}"
        );
    }
}

#[tokio::test]
async fn tampered_ciphertext_fails() {
    let custodian = full_keyring();
    let sealed = alice_envelope(&custodian).await;
    let body_start = nonce_start(&sealed) + NONCE_BYTES;
    let tag_start = sealed.as_ref().len() - AEAD_TAG_BYTES;

    for index in body_start..tag_start {
        let opened = custodian
            .open(
                SealPurpose::DidCustody,
                &subject("account_keys"),
                &flipped(&sealed, index),
            )
            .await
            .err();

        assert_eq!(
            opened,
            Some(CustodyError::Unauthentic),
            "ciphertext byte {index}"
        );
    }
}

#[tokio::test]
async fn tampered_tag_fails() {
    let custodian = full_keyring();
    let sealed = alice_envelope(&custodian).await;
    let tag_start = sealed.as_ref().len() - AEAD_TAG_BYTES;

    for index in tag_start..sealed.as_ref().len() {
        let opened = custodian
            .open(
                SealPurpose::DidCustody,
                &subject("account_keys"),
                &flipped(&sealed, index),
            )
            .await
            .err();

        assert_eq!(opened, Some(CustodyError::Unauthentic), "tag byte {index}");
    }
}

#[tokio::test]
async fn truncated_at_every_boundary_fails_without_panic() {
    let custodian = full_keyring();
    let sealed = alice_envelope(&custodian).await;
    let body_start = nonce_start(&sealed) + NONCE_BYTES;

    for length in 0..sealed.as_ref().len() {
        let truncated = SealedEnvelope::from(sealed.as_ref()[..length].to_vec());
        let expected = if length < body_start + AEAD_TAG_BYTES {
            CustodyError::Malformed
        } else {
            CustodyError::Unauthentic
        };

        let opened = custodian
            .open(
                SealPurpose::DidCustody,
                &subject("account_keys"),
                &truncated,
            )
            .await
            .err();

        assert_eq!(opened, Some(expected), "truncated to {length} bytes");
    }
}

#[tokio::test]
async fn a_legacy_blob_starting_with_0x01_fails_closed() {
    let legacy_blob = SealedEnvelope::from(hex_bytes(LEGACY_ALICE_BLOB_STARTING_0X01_HEX));
    let alice: [&[u8]; 2] = [b"account_keys", b"did:plc:alice"];

    let opened = full_keyring()
        .open(SealPurpose::DidCustody, &subject_of(&alice), &legacy_blob)
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::Malformed));
}

#[tokio::test]
async fn versions_other_than_1_are_refused_before_key_lookup() {
    let sealed = alice_envelope(&other_root_keyring()).await;
    let custodian = full_keyring();
    let subject = subject("account_keys");

    for version in u8::MIN..=u8::MAX {
        let mut bytes = sealed.as_ref().to_vec();
        bytes[0] = version;
        let expected = if version == ENVELOPE_VERSION {
            "UnknownKeyId"
        } else {
            "UnknownVersion"
        };

        let opened = custodian
            .open(
                SealPurpose::DidCustody,
                &subject,
                &SealedEnvelope::from(bytes),
            )
            .await
            .err()
            .map(|error| match error {
                CustodyError::UnknownKeyId { .. } => "UnknownKeyId",
                CustodyError::UnknownVersion => "UnknownVersion",
                _ => "another refusal",
            });

        assert_eq!(opened, Some(expected), "version {version:#04x}");
    }
}

#[tokio::test]
async fn a_0_1_legacy_blob_fails_the_v1_opener() {
    let custodian = full_keyring();
    let subject = subject("account_keys");
    let did_custody_key = purpose_key(&test_root(), Purpose::DidCustody);
    let nonce = XNonce::from([0x00; NONCE_BYTES]);
    let payload = Payload {
        msg: b"alice's custody keys".as_slice(),
        aad: subject.as_ref(),
    };
    let legacy_ciphertext = XChaCha20Poly1305::new((&*did_custody_key).into())
        .encrypt(&nonce, payload)
        .expect("the legacy shape seals");
    let legacy_blob = [nonce.as_slice(), legacy_ciphertext.as_slice()].concat();

    let opened = custodian
        .open(
            SealPurpose::DidCustody,
            &subject,
            &SealedEnvelope::from(legacy_blob),
        )
        .await
        .err();

    assert_eq!(opened, Some(CustodyError::UnknownVersion));
}

#[tokio::test]
async fn sealing_twice_gives_different_nonces() {
    let custodian = full_keyring();
    let first = alice_envelope(&custodian).await;
    let second = alice_envelope(&custodian).await;
    let start = nonce_start(&first);

    let first_nonce = &first.as_ref()[start..start + NONCE_BYTES];
    let second_nonce = &second.as_ref()[start..start + NONCE_BYTES];

    assert_ne!(first_nonce, second_nonce);
}

#[tokio::test]
async fn a_seal_purpose_key_id_is_unknown_to_verify() {
    let custodian = test_keyring(&[Purpose::DidCustody, Purpose::DenMac]);
    let message = mac_message(0);
    let tag = custodian
        .mac(MacUse::DenNode, &message)
        .await
        .expect("the keyring tags");
    let did_custody_key_id = custodian
        .seal_key(SealPurpose::DidCustody)
        .expect("held")
        .key_id
        .clone();
    assert!(did_custody_key_id.as_ref().starts_with("hkdf:did_custody:"));
    let relabelled =
        MacTag::try_from((did_custody_key_id.clone(), tag.stored_bytes())).expect("a 32-byte tag");
    let unknown_key_id = CustodyError::UnknownKeyId {
        key_id: did_custody_key_id,
    };

    let verified = custodian
        .verify(MacUse::DenNode, &message, &relabelled)
        .await;

    assert_eq!(verified, Err(unknown_key_id));
}

#[tokio::test]
async fn a_keyring_never_serves_a_purpose_it_was_not_built_with() {
    let full = full_keyring();
    let signin_only = test_keyring(&[Purpose::SigninSecrets]);
    let subject = subject("account_keys");
    let message = mac_message(0);
    let sealed = alice_envelope(&full).await;
    let tag = full
        .mac(MacUse::DenNode, &message)
        .await
        .expect("the full keyring tags");
    let did_custody_not_held = CustodyError::PurposeNotHeld {
        purpose: Purpose::DidCustody,
    };
    let den_mac_not_held = CustodyError::PurposeNotHeld {
        purpose: Purpose::DenMac,
    };

    let sealing = signin_only
        .seal(SealPurpose::DidCustody, &subject, b"keys")
        .await
        .err();
    let opening = signin_only
        .open(SealPurpose::DidCustody, &subject, &sealed)
        .await
        .err();
    let tagging = signin_only.mac(MacUse::DenNode, &message).await.err();
    let verifying = signin_only.verify(MacUse::DenNode, &message, &tag).await;

    assert_eq!(sealing, Some(did_custody_not_held.clone()));
    assert_eq!(opening, Some(did_custody_not_held));
    assert_eq!(tagging, Some(den_mac_not_held.clone()));
    assert_eq!(verifying, Err(den_mac_not_held));
}

#[tokio::test]
async fn keyring_without_did_custody_refuses_it() {
    let custodian = test_keyring(&[Purpose::SigninSecrets, Purpose::DenMac]);
    let subject = subject("account_keys");
    let sealed = alice_envelope(&full_keyring()).await;
    let did_custody_not_held = CustodyError::PurposeNotHeld {
        purpose: Purpose::DidCustody,
    };

    let sealing = custodian
        .seal(SealPurpose::DidCustody, &subject, b"keys")
        .await
        .err();
    let opening = custodian
        .open(SealPurpose::DidCustody, &subject, &sealed)
        .await
        .err();
    let signin_sealing = custodian
        .seal(SealPurpose::SigninSecrets, &subject, b"secret")
        .await;

    assert_eq!(sealing, Some(did_custody_not_held.clone()));
    assert_eq!(opening, Some(did_custody_not_held));
    assert!(signin_sealing.is_ok());
}

#[test]
fn a_repeated_purpose_is_held_once() {
    let custodian = test_keyring(&[Purpose::DidCustody, Purpose::DidCustody]);

    assert_eq!(custodian.held_purposes(), [Purpose::DidCustody]);
}

#[test]
fn debug_prints_the_purposes_and_no_root_key_or_key_id() {
    let root = test_root();
    let custodian = full_keyring();
    let keys: Vec<Vec<u8>> = Purpose::VARIANTS
        .iter()
        .map(|&purpose| purpose_key(&root, purpose).to_vec())
        .chain([root.to_vec()])
        .collect();
    let key_ids: Vec<String> = Purpose::VARIANTS
        .iter()
        .map(|&purpose| crate::derivation::key_id(&root, purpose).to_string())
        .collect();
    let engines = [
        base64::engine::general_purpose::STANDARD,
        base64::engine::general_purpose::STANDARD_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ];
    let mut forbidden: Vec<String> = Vec::new();
    for key in &keys {
        forbidden.push(hex_text(key));
        forbidden.extend(engines.iter().map(|engine| engine.encode(key)));
        let rust_form: Vec<String> = key.iter().map(u8::to_string).collect();
        forbidden.push(format!("[{}]", rust_form.join(", ")));
    }
    for key_id in &key_ids {
        let fingerprint = key_id.rsplit(':').next().unwrap_or_default();
        forbidden.push(key_id.clone());
        forbidden.push(fingerprint.to_owned());
    }

    let printed = format!("{custodian:?}");

    assert_eq!(
        printed,
        "HkdfKeyCustodian { purposes: [DidCustody, SigninSecrets, DenMac], .. }"
    );
    for text in &forbidden {
        assert!(!printed.contains(text.as_str()), "Debug printed a secret");
    }
}
