use std::str::FromStr;

use super::*;

#[test]
fn a_token_round_trips_through_its_text() {
    let text = "010000000000000032";

    let token = PageToken::from_str(text).expect("a well-formed token");

    assert_eq!(token.to_string(), text);
    assert_eq!(token.into_inner().offset(), 50);
}

#[test]
fn malformed_text_is_the_page_token_error() {
    for text in ["", "zz", "020000000000000032", "01000000000000003G"] {
        let parsed = PageToken::from_str(text);

        assert_eq!(parsed, Err(PageTokenError), "{text:?}");
    }
}
