use super::*;

/// Every fixed word the Den names a node with.
const WORDS: [&str; 10] = [
    ACCOUNTS,
    CHARACTERS,
    COMMISSIONS,
    WORKFLOWS,
    ATTACHMENTS,
    CHANGELOG,
    PRODUCTS,
    SLOTS,
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
