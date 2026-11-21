use super::*;

/// A key id whose fingerprint no `Debug` may print.
const PLANTED_KEY_ID: &str = "hkdf:den_mac:0123456789abcdef";

/// The fingerprint part of [`PLANTED_KEY_ID`].
const PLANTED_FINGERPRINT: &str = "0123456789abcdef";

/// Secret bytes that must appear in no printed form.
const PLANTED_SECRET: &[u8] = b"planted-secret-7f3a";

fn planted_key_id() -> KeyId {
    KeyId::try_from(PLANTED_KEY_ID).expect("a valid key id")
}

/// Every printing of `bytes` this suite looks for: as text, as `{:?}` of the slice, as
/// lowercase and uppercase hex.
fn printed_forms(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let as_list = format!("{bytes:?}");
    let lower_hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let upper_hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
    vec![text, as_list, lower_hex, upper_hex]
}

fn assert_prints_nothing_of(printed: &str, bytes: &[u8]) {
    for form in printed_forms(bytes) {
        assert!(!printed.contains(&form), "{printed} prints {form}");
    }
}

#[test]
fn a_key_id_is_one_to_255_bytes_of_its_alphabet() {
    let longest = "a".repeat(255);

    for good in [
        "a",
        "hkdf:did_custody:0123456789abcdef",
        "legacy-0_1:x",
        longest.as_str(),
    ] {
        assert!(KeyId::try_from(good).is_ok(), "{good:?}");
    }
}

#[test]
fn a_key_id_outside_its_length_or_alphabet_is_refused() {
    let too_long = "a".repeat(256);
    let cases = [
        ("", KeyIdError::Empty),
        (too_long.as_str(), KeyIdError::TooLong),
        ("hkdf:DID_custody:00", KeyIdError::OutsideAlphabet),
        ("hkdf:did custody:00", KeyIdError::OutsideAlphabet),
        ("hkdf:did_custody:00\n", KeyIdError::OutsideAlphabet),
        ("hkdf/did_custody", KeyIdError::OutsideAlphabet),
        ("hkdf:did_cüstody:00", KeyIdError::OutsideAlphabet),
    ];

    for (bad, expected) in cases {
        assert_eq!(KeyId::try_from(bad), Err(expected), "{bad:?}");
    }
}

#[test]
fn a_key_id_prints_whole_and_reads_back() {
    let key_id = planted_key_id();

    assert_eq!(key_id.to_string(), PLANTED_KEY_ID);
    assert_eq!(key_id.as_ref(), PLANTED_KEY_ID);
}

#[test]
fn a_key_id_s_purpose_is_the_token_between_its_first_two_colons() {
    let cases = [
        (
            "hkdf:did_custody:0123456789abcdef",
            Some(Purpose::DidCustody),
        ),
        ("hkdf:signin_secrets:0123", Some(Purpose::SigninSecrets)),
        ("hkdf:den_mac:", Some(Purpose::DenMac)),
        ("hkdf:den_mac:a:b", Some(Purpose::DenMac)),
        ("hkdf:den_mac", None),
        ("den_mac:x:y", None),
        ("hkdf::den_mac", None),
        ("hkdf:root:0123", None),
        ("hkdf", None),
    ];

    for (text, expected) in cases {
        let key_id = KeyId::try_from(text).expect("a valid key id");

        assert_eq!(key_id.purpose(), expected, "{text:?}");
    }
}

#[test]
fn a_key_id_s_debug_prints_its_purpose_only() {
    let printed = format!("{:?}", planted_key_id());

    assert_eq!(printed, "KeyId { purpose: Some(DenMac), .. }");
}

#[test]
fn parts_encode_as_a_count_then_each_length_and_bytes() {
    let parts: &[&[u8]] = &[b"ab", b""];
    let subject = CustodySubject::try_from(parts).expect("small parts encode");

    let expected: &[u8] = &[0, 0, 0, 2, 0, 0, 0, 2, b'a', b'b', 0, 0, 0, 0];
    assert_eq!(subject.as_ref(), expected);
}

#[test]
fn distinct_parts_encode_distinctly() {
    let part_lists: [&[&[u8]]; 6] = [
        &[b"ab", b"c"],
        &[b"a", b"bc"],
        &[b"abc"],
        &[b"abc", b""],
        &[b"", b"abc"],
        &[],
    ];
    let subjects: Vec<Vec<u8>> = part_lists
        .iter()
        .map(|parts| {
            CustodySubject::try_from(*parts)
                .expect("small parts encode")
                .as_ref()
                .to_vec()
        })
        .collect();
    let messages: Vec<Vec<u8>> = part_lists
        .iter()
        .map(|parts| {
            MacMessage::try_from(*parts)
                .expect("small parts encode")
                .as_ref()
                .to_vec()
        })
        .collect();

    for (left, left_subject) in subjects.iter().enumerate() {
        for (right, right_subject) in subjects.iter().enumerate().skip(left + 1) {
            assert_ne!(left_subject, right_subject, "lists {left} and {right}");
        }
    }
    assert_eq!(messages, subjects);
}

#[test]
fn an_oversize_length_is_refused_not_truncated() {
    let just_too_long = u32::MAX as usize + 1;

    assert!(length_prefix(just_too_long).is_err());
    assert!(length_prefix(usize::MAX).is_err());
    assert_eq!(length_prefix(u32::MAX as usize), Ok([0xff; 4]));
    assert_eq!(length_prefix(258), Ok([0, 0, 1, 2]));
}

#[test]
fn a_subject_and_a_message_print_their_length_only() {
    let parts: &[&[u8]] = &[PLANTED_SECRET, b"did:plc:planted"];
    let subject = CustodySubject::try_from(parts).expect("small parts encode");
    let message = MacMessage::try_from(parts).expect("small parts encode");

    let printed_subject = format!("{subject:?}");
    let printed_message = format!("{message:?}");

    assert_eq!(printed_subject, "CustodySubject { bytes: 46, .. }");
    assert_eq!(printed_message, "MacMessage { bytes: 46, .. }");
    assert_prints_nothing_of(&printed_subject, PLANTED_SECRET);
    assert_prints_nothing_of(&printed_message, PLANTED_SECRET);
    assert_prints_nothing_of(&printed_subject, b"planted");
}

/// A version-1 envelope under `key_id` holding `ciphertext`.
fn envelope(key_id: &str, ciphertext: &[u8]) -> Vec<u8> {
    let key_id_length = u8::try_from(key_id.len()).expect("a key id fits a byte");
    let mut bytes = vec![ENVELOPE_VERSION, key_id_length];
    bytes.extend_from_slice(key_id.as_bytes());
    bytes.extend_from_slice(ciphertext);
    bytes
}

#[test]
fn an_envelope_prints_its_length_and_header_purpose_only() {
    let stored = envelope(PLANTED_KEY_ID, PLANTED_SECRET);
    let length = stored.len();
    let sealed = SealedEnvelope::from(stored);

    let printed = format!("{sealed:?}");

    let expected = format!("SealedEnvelope {{ bytes: {length}, purpose: Some(DenMac) }}");
    assert_eq!(printed, expected);
    assert!(!printed.contains(PLANTED_FINGERPRINT), "{printed}");
    assert_prints_nothing_of(&printed, PLANTED_SECRET);
}

#[test]
fn an_envelope_whose_header_doesn_t_parse_prints_no_purpose() {
    let mut other_version = envelope(PLANTED_KEY_ID, PLANTED_SECRET);
    other_version[0] = 0x02;
    let mut cut_short = envelope(PLANTED_KEY_ID, b"");
    cut_short.truncate(10);
    let bad_key_id = envelope("hkdf:DEN_MAC:00", PLANTED_SECRET);
    let mut not_text = envelope("hkdf", b"");
    not_text[2] = 0xff;

    for stored in [
        vec![],
        vec![ENVELOPE_VERSION],
        other_version,
        cut_short,
        bad_key_id,
        not_text,
    ] {
        let length = stored.len();
        let sealed = SealedEnvelope::from(stored);

        let printed = format!("{sealed:?}");

        let expected = format!("SealedEnvelope {{ bytes: {length}, purpose: None }}");
        assert_eq!(printed, expected);
        assert_prints_nothing_of(&printed, PLANTED_SECRET);
    }
}

#[test]
fn an_envelope_reads_back_its_stored_bytes() {
    let stored = envelope(PLANTED_KEY_ID, PLANTED_SECRET);
    let sealed = SealedEnvelope::from(stored.clone());

    assert_eq!(sealed.as_ref(), stored.as_slice());
}

#[test]
fn a_mac_tag_is_32_bytes() {
    let tag_bytes = [7u8; 32];

    let tag = MacTag::try_from((planted_key_id(), tag_bytes.as_slice())).expect("32 bytes");

    assert_eq!(tag.key_id(), &planted_key_id());
    assert_eq!(tag.stored_bytes(), tag_bytes.as_slice());
}

#[test]
fn a_mac_tag_of_another_length_is_refused() {
    for length in [0, 31, 33, 64] {
        let bytes = vec![7u8; length];

        let refused = MacTag::try_from((planted_key_id(), bytes.as_slice()));

        let wrong_length = MacTagError::WrongLength { length };
        assert_eq!(refused.err(), Some(wrong_length));
    }
}

#[test]
fn a_mac_tag_prints_its_purpose_and_length_only() {
    let tag_bytes = [0xabu8; 32];
    let tag = MacTag::try_from((planted_key_id(), tag_bytes.as_slice())).expect("32 bytes");

    let printed = format!("{tag:?}");

    assert_eq!(printed, "MacTag { purpose: Some(DenMac), tag_bytes: 32 }");
    assert!(!printed.contains(PLANTED_FINGERPRINT), "{printed}");
    assert_prints_nothing_of(&printed, &tag_bytes);
}

#[test]
fn an_opened_secret_prints_nothing_and_exposes_its_bytes() {
    let secret = OpenedSecret::from(PLANTED_SECRET.to_vec());

    let printed = format!("{secret:?}");

    assert_eq!(printed, "OpenedSecret { .. }");
    assert_prints_nothing_of(&printed, PLANTED_SECRET);
    assert_eq!(secret.expose(), PLANTED_SECRET);
}
