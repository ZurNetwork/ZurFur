use super::*;
use crate::elements::commission::file::FileNameError;

#[test]
fn filename_trims_and_rejects_unsafe_input() {
    assert_eq!(
        FileName::try_from(" sketch.png ").unwrap().as_str(),
        "sketch.png"
    );
    // A bare unicode name is fine (served RFC 5987-encoded).
    assert!(FileName::try_from("café.png").is_ok());
    assert_eq!(FileName::try_from("   "), Err(FileNameError::Empty));
    assert_eq!(
        FileName::try_from("x".repeat(FileName::MAX_BYTES + 1)),
        Err(FileNameError::TooLong)
    );
    // Exactly at the cap is fine.
    assert!(FileName::try_from("x".repeat(FileName::MAX_BYTES)).is_ok());
    for bad in ["a\nb.png", "a\tb.png", "a\rb.png", "a\0b.png"] {
        assert_eq!(
            FileName::try_from(bad),
            Err(FileNameError::ControlCharacter),
            "control characters are rejected: {bad:?}",
        );
    }
    for bad in ["../secret", "dir/file.png", "dir\\file.png"] {
        assert_eq!(
            FileName::try_from(bad),
            Err(FileNameError::PathSeparator),
            "path separators are rejected: {bad:?}",
        );
    }
}

#[test]
fn filename_refuses_a_leading_dot() {
    for bad in [".theme.yaml", ".", "..", "  .theme.yaml"] {
        assert_eq!(
            FileName::try_from(bad),
            Err(FileNameError::LeadingDot),
            "a dotfile is refused: {bad:?}",
        );
    }
}

#[test]
fn filename_accepts_a_dot_anywhere_but_first() {
    for good in ["theme.yaml", "a.b.png", "x."] {
        let filename = FileName::try_from(good).expect("an interior or trailing dot is fine");

        assert_eq!(filename.as_str(), good);
    }
}
