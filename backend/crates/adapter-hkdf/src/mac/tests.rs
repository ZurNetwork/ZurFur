use domain::elements::custody::{MacTagError, Purpose};
use strum::VariantArray;

use super::*;
use crate::derivation::{key_id, purpose_key};
use crate::golden::{array_at, bytes_at, golden, hex_text, mac_message, test_root, text_at};

/// The Den MAC key the test root derives.
fn den_mac_key() -> DenMacKey {
    let root = test_root();
    let key = purpose_key(&root, Purpose::DenMac);
    DenMacKey::from((key_id(&root, Purpose::DenMac), &*key))
}

/// A message no golden tag was computed over.
fn other_message() -> MacMessage {
    let parts: [&[u8]; 2] = [b"node", b"another"];
    MacMessage::try_from(parts.as_slice()).expect("a short message encodes")
}

#[test]
fn mac_tags_match_the_golden_file() {
    let golden = golden();
    let den_mac_key = den_mac_key();

    for (index, message) in array_at(&golden, "/mac_messages").iter().enumerate() {
        let mac_use: MacUse = text_at(message, "/use").parse().expect("a golden MAC use");
        let expected_tag = bytes_at(message, "/tag");

        let tag = den_mac_key
            .tag(mac_use, &mac_message(index))
            .expect("a golden message tags");

        assert_eq!(hex_text(tag.stored_bytes()), hex_text(&expected_tag));
        assert_eq!(tag.key_id(), &den_mac_key.key_id);
    }
}

#[test]
fn every_use_is_in_the_golden_file() {
    let golden = golden();
    let golden_uses: Vec<&str> = array_at(&golden, "/mac_messages")
        .iter()
        .map(|message| text_at(message, "/use"))
        .collect();

    for &mac_use in MacUse::VARIANTS {
        let token: &'static str = mac_use.into();
        assert!(golden_uses.contains(&token), "{token} has no golden tag");
    }
}

#[test]
fn a_tag_verifies_for_its_own_use_and_message() {
    let den_mac_key = den_mac_key();
    let message = mac_message(0);

    let tag = den_mac_key
        .tag(MacUse::DenNode, &message)
        .expect("a message tags");

    assert_eq!(den_mac_key.verify(MacUse::DenNode, &message, &tag), Ok(()));
}

#[test]
fn den_node_tag_wont_verify_as_changelog_entry() {
    let den_mac_key = den_mac_key();
    let message = mac_message(0);
    let tag = den_mac_key
        .tag(MacUse::DenNode, &message)
        .expect("a message tags");

    let verified = den_mac_key.verify(MacUse::DenChangelogEntry, &message, &tag);

    assert_eq!(verified, Err(CustodyError::Unauthentic));
}

#[test]
fn a_tampered_message_fails() {
    let den_mac_key = den_mac_key();
    let tag = den_mac_key
        .tag(MacUse::DenNode, &mac_message(0))
        .expect("a message tags");

    let verified = den_mac_key.verify(MacUse::DenNode, &other_message(), &tag);

    assert_eq!(verified, Err(CustodyError::Unauthentic));
}

#[test]
fn a_tampered_tag_fails() {
    let den_mac_key = den_mac_key();
    let message = mac_message(0);
    let tag = den_mac_key
        .tag(MacUse::DenNode, &message)
        .expect("a message tags");

    for index in 0..MAC_TAG_BYTES {
        let mut tampered_bytes = tag.stored_bytes().to_vec();
        tampered_bytes[index] ^= 0x01;
        let tampered = MacTag::try_from((tag.key_id().clone(), tampered_bytes.as_slice()))
            .expect("a 32-byte tag");

        let verified = den_mac_key.verify(MacUse::DenNode, &message, &tampered);

        assert_eq!(verified, Err(CustodyError::Unauthentic), "byte {index}");
    }
}

#[test]
fn a_wrong_length_tag_is_refused() {
    let den_mac_key = den_mac_key();
    let tag = den_mac_key
        .tag(MacUse::DenNode, &mac_message(0))
        .expect("a message tags");

    for length in [0, MAC_TAG_BYTES - 1, MAC_TAG_BYTES + 1] {
        let bytes = vec![0u8; length];
        let wrong_length = MacTagError::WrongLength { length };

        let built = MacTag::try_from((tag.key_id().clone(), bytes.as_slice())).err();

        assert_eq!(built, Some(wrong_length));
    }
}

#[test]
fn a_tag_under_another_key_id_is_unknown() {
    let den_mac_key = den_mac_key();
    let message = mac_message(0);
    let tag = den_mac_key
        .tag(MacUse::DenNode, &message)
        .expect("a message tags");
    let other_key_id = KeyId::try_from("hkdf:den_mac:0000000000000000").expect("a key id");
    let relabelled =
        MacTag::try_from((other_key_id.clone(), tag.stored_bytes())).expect("a 32-byte tag");
    let unknown_key_id = CustodyError::UnknownKeyId {
        key_id: other_key_id,
    };

    let verified = den_mac_key.verify(MacUse::DenNode, &message, &relabelled);

    assert_eq!(verified, Err(unknown_key_id));
}
