use super::*;

/// A key id whose fingerprint no message may print.
const PLANTED_KEY_ID: &str = "hkdf:did_custody:0123456789abcdef";

/// The fingerprint part of [`PLANTED_KEY_ID`].
const PLANTED_FINGERPRINT: &str = "0123456789abcdef";

fn every_variant() -> Vec<CustodyError> {
    let key_id = KeyId::try_from(PLANTED_KEY_ID).expect("a valid key id");
    vec![
        CustodyError::PurposeNotHeld {
            purpose: Purpose::DenMac,
        },
        CustodyError::UnknownVersion,
        CustodyError::Malformed,
        CustodyError::UnknownKeyId { key_id },
        CustodyError::Unauthentic,
        CustodyError::NotRewrapped,
    ]
}

#[test]
fn each_message_names_its_kind() {
    let messages: Vec<String> = every_variant()
        .iter()
        .map(|error| error.to_string())
        .collect();

    let expected = [
        "no custody key is held for the purpose den_mac",
        "the envelope's version is unknown",
        "the envelope is malformed",
        "the key id names no key the custodian holds",
        "the envelope or tag is not authentic",
        "the stored value has not been rewrapped",
    ];
    assert_eq!(messages, expected);
}

#[test]
fn no_message_prints_the_key_id() {
    for error in every_variant() {
        let message = error.to_string();
        let debug = format!("{error:?}");

        assert!(!message.contains(PLANTED_FINGERPRINT), "{message}");
        assert!(!message.contains("hkdf"), "{message}");
        assert!(!debug.contains(PLANTED_FINGERPRINT), "{debug}");
    }
}
