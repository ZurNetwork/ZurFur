use super::*;
use crate::elements::commission::file::FileName;

#[test]
fn content_type_is_normalized_to_a_safe_header_value() {
    let name = FileName::try_new("art.svg").unwrap();
    // A good MIME is kept verbatim (trimmed).
    assert_eq!(
        FileMetadata::new(name.clone(), "  image/svg+xml  ", 10).content_type,
        "image/svg+xml"
    );
    // Blank or control-bearing MIME falls back to octet-stream (no injection).
    assert_eq!(
        FileMetadata::new(name.clone(), "   ", 10).content_type,
        FileMetadata::DEFAULT_CONTENT_TYPE
    );
    assert_eq!(
        FileMetadata::new(name, "text/html\r\nSet-Cookie: x", 10).content_type,
        FileMetadata::DEFAULT_CONTENT_TYPE
    );
}
