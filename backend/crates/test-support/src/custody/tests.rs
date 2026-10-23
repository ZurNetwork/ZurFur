use super::*;

/// RFC 5869's test case 1: its input key, salt, info, PRK and 42 output bytes.
const RFC_5869_IKM: [u8; 22] = [0x0b; 22];
const RFC_5869_SALT: &str = "000102030405060708090a0b0c";
const RFC_5869_INFO: &str = "f0f1f2f3f4f5f6f7f8f9";
const RFC_5869_PRK: &str = "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5";
const RFC_5869_OKM: &str =
    "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865";

/// Each door subject's name in the golden file.
const DOOR_SUBJECTS: [&str; 3] = ["account_keys", "client_session", "auth_request"];

/// The hex parts of the golden subject named `name`, as text.
fn golden_parts(name: &str) -> Vec<String> {
    let golden = golden_file();
    let pointer = format!("/subjects/{name}/parts");
    let parts = golden
        .pointer(&pointer)
        .and_then(serde_json::Value::as_array)
        .expect("the golden file has the subject");
    parts
        .iter()
        .map(|part| part.as_str().expect("hex text").to_owned())
        .collect()
}

#[test]
fn each_door_subject_builds_to_the_golden_encoding() {
    let golden = golden_file();

    for name in DOOR_SUBJECTS {
        let subject = golden_subject(name);

        let encoding_pointer = format!("/subjects/{name}/encoding");
        let expected = golden
            .pointer(&encoding_pointer)
            .and_then(serde_json::Value::as_str)
            .expect("the golden file has the encoding");
        assert_eq!(HEXLOWER.encode(subject.as_ref()), expected, "{name}");
    }
}

#[test]
#[should_panic(expected = "no_such_subject")]
fn an_unknown_subject_name_panics_with_the_name() {
    golden_subject("no_such_subject");
}

#[test]
fn the_golden_inputs_are_parts_of_their_subjects() {
    let did = HEXLOWER.encode(GOLDEN_DID.as_bytes());
    let session_id = HEXLOWER.encode(GOLDEN_SESSION_ID.as_bytes());
    let state = HEXLOWER.encode(GOLDEN_STATE.as_bytes());

    assert!(golden_parts("account_keys").contains(&did));
    assert!(golden_parts("client_session").contains(&did));
    assert!(golden_parts("client_session").contains(&session_id));
    assert!(golden_parts("auth_request").contains(&state));
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
    let golden = golden_file();
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

    assert_eq!(
        envelope_key_id(did_custody.as_ref()),
        key_id_of(&root, Purpose::DidCustody)
    );
    assert_eq!(
        envelope_key_id(signin_secrets.as_ref()),
        key_id_of(&root, Purpose::SigninSecrets)
    );
    assert_eq!(tag.key_id().as_ref(), key_id_of(&root, Purpose::DenMac));
}

#[test]
fn printed_forms_are_every_hex_base64_and_byte_list_form() {
    let forms = printed_forms(&[0x00, 0x7f, 0xff, 0x10]);

    let forms: Vec<(&str, &str)> = forms
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let expected = [
        ("as hex", "007fff10"),
        ("as uppercase hex", "007FFF10"),
        ("as base64", "AH//EA=="),
        ("as unpadded base64", "AH//EA"),
        ("as URL-safe base64", "AH__EA=="),
        ("as unpadded URL-safe base64", "AH__EA"),
        ("as a byte list", "[0, 127, 255, 16]"),
        ("as a hex byte list", "[0, 7f, ff, 10]"),
        ("as an uppercase hex byte list", "[0, 7F, FF, 10]"),
    ];
    assert_eq!(forms, expected);
}

#[test]
fn nine_bytes_print_two_windows_in_each_case() {
    let bytes = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe];

    let forms = printed_forms(&bytes);

    let windows: Vec<(&str, &str)> = forms
        .iter()
        .filter(|(name, _)| name.starts_with("bytes "))
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let expected = [
        ("bytes 0..8 as hex", "0123456789abcdef"),
        ("bytes 0..8 as uppercase hex", "0123456789ABCDEF"),
        ("bytes 1..9 as hex", "23456789abcdeffe"),
        ("bytes 1..9 as uppercase hex", "23456789ABCDEFFE"),
    ];
    assert_eq!(windows, expected);
}

#[test]
fn fewer_than_eight_bytes_print_no_window() {
    let forms = printed_forms(&[0x01; 7]);

    let windows = forms.iter().filter(|(name, _)| name.starts_with("bytes "));
    assert_eq!(windows.count(), 0);
}

#[test]
fn named_printings_name_the_whole_value_and_each_window() {
    let printings = named_printings("the signing key", &[0x01; 9]);

    let names: Vec<&str> = printings.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"the signing key as uppercase hex"));
    assert!(names.contains(&"the signing key's bytes 1..9 as hex"));
    assert_eq!(printings.len(), 9 + 2 * 2);
}

#[test]
fn derived_printings_cover_the_prk_and_every_purpose_key() {
    let root = [0x21; KEY_BYTES];

    let printings = derived_printings(&root);

    let prk_hex = HEXLOWER.encode(&prk_of(&root));
    let windows_per_key = KEY_BYTES - 8 + 1;
    let forms_per_key = 9 + 2 * windows_per_key;
    let expected_printings = forms_per_key * (1 + Purpose::VARIANTS.len());
    assert_eq!(printings.len(), expected_printings);
    assert!(printings.iter().any(|(_, printed)| *printed == prk_hex));
    for purpose in Purpose::VARIANTS {
        let key_base64 = BASE64.encode(&purpose_key_of(&root, *purpose));
        let printed = printings.iter().any(|(_, printed)| *printed == key_base64);
        assert!(printed, "{purpose}");
    }
}
