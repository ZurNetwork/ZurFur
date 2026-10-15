use super::*;

#[test]
fn upload_file_response_serializes_to_a_bare_id_object() {
    let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000006").unwrap();
    let body = UploadFileResponse { id };

    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        format!("{{\"id\":\"{id}\"}}")
    );
}

#[test]
fn content_disposition_is_attachment_and_encodes_safely() {
    let value = content_disposition(&save_name("réf sheet.png"));
    assert!(
        value.starts_with("attachment; "),
        "always attachment: {value}"
    );
    assert!(
        value.contains("filename*=UTF-8''r%C3%A9f%20sheet.png"),
        "{value}"
    );
    assert!(value.contains("filename=\"r_f sheet.png\""), "{value}");
}

#[test]
fn rfc5987_leaves_attr_chars_and_escapes_the_rest() {
    assert_eq!(rfc5987_encode("a-b_c.png"), "a-b_c.png");
    assert_eq!(rfc5987_encode("a b"), "a%20b");
    assert_eq!(rfc5987_encode("\""), "%22");
}

/// The save name `text`, which the Den's name rule accepts.
fn save_name(text: &str) -> DenName {
    DenName::try_from(text).expect("the Den's name rule accepts it")
}

// The Den's name rule refuses control characters and bidi overrides before a
// name reaches the header builder, so `DenName` is the barrier; the builder only
// encodes. The value is a valid header, the quoted fallback holds only printable
// ASCII without `"`, `\` or `%`, and `filename*` percent-encodes every byte
// outside RFC 5987's `attr-char` set.
#[test]
fn the_den_s_name_rule_refuses_what_would_disguise_or_split_the_header() {
    for hostile in [
        "a\r\nSet-Cookie: x",
        "photo\u{202E}gpj.exe",
        "a\u{1}\u{7f}b",
    ] {
        assert!(DenName::try_from(hostile).is_err(), "{hostile:?}");
    }
}

#[test]
fn content_disposition_encodes_quotes_backslashes_and_semicolons_into_one_valid_header() {
    let awkward = save_name("x\"\\; réf.png");

    let value = content_disposition(&awkward);

    assert!(
        HeaderValue::from_str(&value).is_ok(),
        "a valid header value: {value:?}"
    );
    assert!(value.is_ascii(), "nothing outside ASCII: {value:?}");
    let expected =
        "attachment; filename=\"x__; r_f.png\"; filename*=UTF-8''x%22%5C%3B%20r%C3%A9f.png";
    assert_eq!(value, expected);
}

// RFC 6266 Appendix D: a `%` and two hex digits in `filename` may be decoded as
// an escape by a user agent that ignores `filename*`.
#[test]
fn content_disposition_keeps_percent_escapes_out_of_the_quoted_fallback() {
    let spelled_out = save_name("photo%E2%80%AEgpj.exe");

    let value = content_disposition(&spelled_out);

    let expected = "attachment; filename=\"photo_E2_80_AEgpj.exe\"; filename*=UTF-8''photo%25E2%2580%25AEgpj.exe";
    assert_eq!(value, expected);
}
