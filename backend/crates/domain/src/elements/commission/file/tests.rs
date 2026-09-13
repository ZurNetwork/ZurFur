use super::*;

#[test]
fn filename_trims_and_rejects_unsafe_input() {
    assert_eq!(
        FileName::try_new(" sketch.png ").unwrap().as_str(),
        "sketch.png"
    );
    // A bare unicode name is fine (served RFC 5987-encoded).
    assert!(FileName::try_new("café.png").is_ok());
    assert_eq!(FileName::try_new("   "), Err(FileNameError::Empty));
    assert_eq!(
        FileName::try_new("x".repeat(FileName::MAX_BYTES + 1)),
        Err(FileNameError::TooLong)
    );
    // Exactly at the cap is fine.
    assert!(FileName::try_new("x".repeat(FileName::MAX_BYTES)).is_ok());
    for bad in ["a\nb.png", "a\tb.png", "a\rb.png", "a\0b.png"] {
        assert_eq!(
            FileName::try_new(bad),
            Err(FileNameError::ControlCharacter),
            "control characters are rejected: {bad:?}",
        );
    }
    for bad in ["../secret", "dir/file.png", "dir\\file.png"] {
        assert_eq!(
            FileName::try_new(bad),
            Err(FileNameError::PathSeparator),
            "path separators are rejected: {bad:?}",
        );
    }
}

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

#[test]
fn a_file_key_is_opaque_and_round_trips() {
    let raw = uuid::Uuid::now_v7();
    assert_eq!(*FileKey::new(raw), raw);
    assert_ne!(*FileKey::generate(), *FileKey::generate());
}
