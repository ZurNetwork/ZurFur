use super::*;
use crate::elements::commission::file::FileNameError;

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
