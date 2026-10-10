use super::*;

// Loading keeps the text byte for byte: nothing is trimmed or refused.
#[test]
fn stored_text_keeps_what_the_store_holds() {
    let raw = " \u{202E}gnp.exe \t".to_owned();
    let stored = StoredText::from(raw.clone());
    assert_eq!(stored.as_str(), raw);
}

// The empty string loads too: a reader decides what to show for it.
#[test]
fn stored_text_loads_the_empty_string() {
    let stored = StoredText::from(String::new());
    assert_eq!(stored.as_str(), "");
}
