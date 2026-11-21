use super::*;

/// Every fixed word the Den names a node with.
const WORDS: [&str; 12] = [
    ACCOUNTS,
    CHARACTERS,
    COMMISSIONS,
    WORKFLOWS,
    ATTACHMENTS,
    CHANGELOG,
    PRODUCTS,
    SLOTS,
    PARTICIPANTS,
    BRIEF,
    POSTS,
    UNNAMED,
];

#[test]
fn every_fixed_word_is_a_segment_and_a_name() {
    for word in WORDS {
        let segment = fixed_segment(word);
        let name = fixed_name(word);

        assert_eq!(segment.as_ref().map(AsRef::as_ref), Some(word), "{word}");
        assert_eq!(name.as_ref().map(AsRef::as_ref), Some(word), "{word}");
    }
}

#[test]
fn stored_text_that_passes_the_name_rule_is_its_own_name() {
    let name = stored_name("Untitled");

    assert_eq!(name.as_ref().map(AsRef::as_ref), Some("Untitled"));
}

#[test]
fn stored_text_that_fails_the_name_rule_is_unnamed() {
    for text in ["", "a/b", "\u{202E}", "\u{200D}", "line\u{2028}break"] {
        let name = stored_name(text);

        assert_eq!(name.as_ref().map(AsRef::as_ref), Some(UNNAMED), "{text:?}");
    }
}

proptest::proptest! {
    #[test]
    fn every_uuid_backed_id_is_a_key(bits in proptest::prelude::any::<u128>()) {
        let uuid = uuid::Uuid::from_u128(bits);
        let commission = domain::elements::commission::CommissionId::from(uuid);
        let element = domain::elements::commission::ElementId::from(uuid);
        let file = domain::elements::commission::FileKey::from(uuid);
        let workflow = domain::elements::workflow::WorkflowId::from(uuid);

        let keys = [
            id_segment(commission),
            id_segment(element),
            id_segment(file),
            id_segment(workflow),
        ];

        let text = uuid.to_string();
        for key in keys {
            proptest::prop_assert_eq!(key.as_ref().map(AsRef::as_ref), Some(text.as_str()));
        }
    }

    #[test]
    fn every_did_the_parser_accepts_is_a_key(
        text in "did:[a-z0-9]{1,8}:[A-Za-z0-9._:%-]{1,2030}"
    ) {
        let did: Did = text.parse().expect("the generator writes valid DIDs");

        let key = did_segment(&did);

        proptest::prop_assert_eq!(key.as_ref().map(AsRef::as_ref), Some(text.as_str()));
    }
}

#[test]
fn a_did_of_2048_bytes_is_a_key_and_a_longer_one_is_not() {
    let prefix = "did:plc:";
    let longest: Did = format!("{prefix}{}", "a".repeat(2048 - prefix.len()))
        .parse()
        .expect("a valid DID");
    let longer: Did = format!("{prefix}{}", "a".repeat(2049 - prefix.len()))
        .parse()
        .expect("a valid DID");

    assert!(did_segment(&longest).is_some());
    assert!(did_segment(&longer).is_none());
}
