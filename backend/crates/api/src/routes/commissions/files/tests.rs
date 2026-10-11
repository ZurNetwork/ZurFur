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
    let value = content_disposition("réf sheet.png");
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

// A stored filename is served as written, so this header builder is the only
// barrier: control characters, quotes, backslashes, `;` and bidi overrides
// never reach the header raw. The value is a valid header, the quoted fallback
// holds only printable ASCII without `"` or `\`, and `filename*` percent-encodes
// every byte outside RFC 5987's `attr-char` set.
#[test]
fn content_disposition_neutralizes_hostile_filenames() {
    let hostile = "a\r\nSet-Cookie: x\"\\;\u{1}\u{7f}\u{202e}gnp.exe";

    let value = content_disposition(hostile);

    assert!(
        HeaderValue::from_str(&value).is_ok(),
        "a valid header value: {value:?}"
    );
    assert!(value.is_ascii(), "nothing outside ASCII: {value:?}");
    assert!(
        !value.chars().any(|c| c.is_ascii_control()),
        "no control characters, CR and LF included: {value:?}"
    );
    let fallback = value
        .strip_prefix("attachment; filename=\"")
        .and_then(|rest| rest.split_once("\"; filename*=UTF-8''"))
        .map(|(fallback, _)| fallback)
        .expect("the attachment, fallback, extended layout");
    assert!(
        fallback.chars().all(|c| c.is_ascii_graphic() || c == ' ')
            && !fallback.contains('"')
            && !fallback.contains('\\'),
        "the quoted fallback is printable ASCII without quotes: {fallback:?}"
    );
    let expected_extended = "a%0D%0ASet-Cookie%3A%20x%22%5C%3B%01%7F%E2%80%AEgnp.exe";
    assert!(
        value.ends_with(&format!("filename*=UTF-8''{expected_extended}")),
        "every unsafe byte percent-encoded: {value:?}"
    );
}
