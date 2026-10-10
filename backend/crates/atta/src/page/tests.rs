use super::*;

#[test]
fn a_token_round_trips_through_its_text() {
    let token = PageToken::at(50);
    let text = token.to_string();
    assert_eq!(text, "010000000000000032");
    assert_eq!(text.parse(), Ok(token));
}

#[test]
fn a_token_holds_only_hex_digits() {
    let text = PageToken::at(usize::MAX).to_string();
    assert!(text.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn malformed_tokens_are_refused() {
    let refused = [
        "",
        "01",
        "0100000000000000320",
        "01000000000000003G",
        "01000000000000003A",
        "+1000000000000003a",
        "01 000000000000032",
    ];
    for text in refused {
        assert_eq!(text.parse::<PageToken>(), Err(PageTokenError), "{text:?}");
    }
}

#[test]
fn a_token_from_another_format_version_is_refused() {
    assert_eq!(
        "020000000000000032".parse::<PageToken>(),
        Err(PageTokenError)
    );
    assert_eq!(
        "000000000000000032".parse::<PageToken>(),
        Err(PageTokenError)
    );
}
