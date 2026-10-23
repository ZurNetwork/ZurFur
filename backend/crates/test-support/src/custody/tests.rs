use super::*;

/// RFC 5869's test case 1: its input key, salt, info, PRK and 42 output bytes.
const RFC_5869_IKM: [u8; 22] = [0x0b; 22];
const RFC_5869_SALT: &str = "000102030405060708090a0b0c";
const RFC_5869_INFO: &str = "f0f1f2f3f4f5f6f7f8f9";
const RFC_5869_PRK: &str = "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5";
const RFC_5869_OKM: &str =
    "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865";

/// The custody golden file adapter-hkdf's script writes.
fn golden() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../adapter-hkdf/golden/custody-v1.json");
    let text = std::fs::read_to_string(&path).expect("the golden file reads");
    serde_json::from_str(&text).expect("the golden file is JSON")
}

#[test]
fn extract_and_expand_meet_rfc_5869_test_case_1() {
    let salt = hex_bytes(RFC_5869_SALT);
    let info = hex_bytes(RFC_5869_INFO);

    let prk = hmac_sha256(&salt, &[&RFC_5869_IKM]);
    let okm = expand(&prk, &info, 42);

    assert_eq!(HEXLOWER.encode(&prk), RFC_5869_PRK);
    assert_eq!(HEXLOWER.encode(&okm), RFC_5869_OKM);
}

#[test]
fn the_canary_derives_the_golden_keys_and_key_ids() {
    let golden = golden();
    let root_bytes = hex_bytes(golden["root"].as_str().expect("the golden root"));
    let root: [u8; KEY_BYTES] = root_bytes.try_into().expect("a 32-byte golden root");

    for purpose in Purpose::VARIANTS {
        let token: &'static str = purpose.into();
        let expected = &golden["keys"][token];

        let key = HEXLOWER.encode(&purpose_key_of(&root, *purpose));
        let key_id = key_id_of(&root, *purpose);

        assert_eq!(Some(key.as_str()), expected["key"].as_str(), "{token}");
        assert_eq!(
            Some(key_id.as_str()),
            expected["key_id"].as_str(),
            "{token}"
        );
    }
}

#[tokio::test]
async fn the_canary_names_the_key_ids_the_custodian_writes() {
    let root = [0x3d; KEY_BYTES];
    let custody = custody_of(root);
    let parts: [&[u8]; 1] = [b"canary"];
    let subject = CustodySubject::try_from(parts.as_slice()).expect("a subject");
    let message = MacMessage::try_from(parts.as_slice()).expect("a message");

    let did_custody = custody
        .did_custody
        .seal(&subject, b"secret")
        .await
        .expect("sealed");
    let signin_secrets = custody
        .signin_secrets
        .seal(&subject, b"secret")
        .await
        .expect("sealed");
    let tag = custody
        .den_mac
        .mac(MacUse::DenNode, &message)
        .await
        .expect("tagged");

    let header_key_id = |envelope: &SealedEnvelope| {
        let [_version, length, rest @ ..] = envelope.as_ref() else {
            panic!("an envelope has a header");
        };
        String::from_utf8(rest[..usize::from(*length)].to_vec()).expect("a key id is text")
    };
    assert_eq!(
        header_key_id(&did_custody),
        key_id_of(&root, Purpose::DidCustody)
    );
    assert_eq!(
        header_key_id(&signin_secrets),
        key_id_of(&root, Purpose::SigninSecrets)
    );
    assert_eq!(tag.key_id().as_ref(), key_id_of(&root, Purpose::DenMac));
}

#[test]
fn printed_forms_are_hex_base64_and_the_byte_list() {
    let forms = printed_forms(&[0x00, 0x7f, 0xff]);

    assert_eq!(forms, ["007fff", "AH//", "[0, 127, 255]"]);
}

#[test]
fn derived_printings_cover_the_prk_and_every_purpose_key() {
    let root = [0x21; KEY_BYTES];

    let printings = derived_printings(&root);

    let prk_hex = HEXLOWER.encode(&prk_of(&root));
    assert_eq!(printings.len(), 3 * (1 + Purpose::VARIANTS.len()));
    assert!(printings.iter().any(|(_, printed)| *printed == prk_hex));
    for purpose in Purpose::VARIANTS {
        let key_base64 = BASE64.encode(&purpose_key_of(&root, *purpose));
        let printed = printings.iter().any(|(_, printed)| *printed == key_base64);
        assert!(printed, "{purpose}");
    }
}
