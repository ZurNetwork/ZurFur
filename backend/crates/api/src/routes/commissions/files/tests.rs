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
