use super::*;

#[test]
fn keys_the_den_uses_parse() {
    let keys = [
        "did:plc:c5vzq2xkzfzj7mqnb3e6h4ty",
        "did:web:example.com%3A8080",
        "01a0ef9c-5b3e-7c1d-9a2f-3e4d5c6b7a89",
        "3lx7kq2ab2c2s",
        "bafkreie5737gdxlw5i64vzichcalba3z2v5n6icifvx5xytvske7mr3hpm",
        "attachments",
        ".theme.yaml",
        "a~b_c-d",
    ];
    for key in keys {
        let parsed = Segment::try_from(key);
        assert_eq!(
            parsed.map(|segment| segment.to_string()),
            Ok(key.to_owned())
        );
    }
}

#[test]
fn the_empty_piece_is_refused() {
    assert_eq!(Segment::try_from(""), Err(SegmentError::Empty));
}

#[test]
fn dot_segments_are_refused() {
    assert_eq!(Segment::try_from("."), Err(SegmentError::DotSegment));
    assert_eq!(Segment::try_from(".."), Err(SegmentError::DotSegment));
}

#[test]
fn a_piece_of_exactly_the_cap_parses_and_one_more_byte_is_refused() {
    let at_cap = "a".repeat(MAX_SEGMENT_BYTES);
    let over_cap = "a".repeat(MAX_SEGMENT_BYTES + 1);
    assert!(Segment::try_from(at_cap).is_ok());
    assert_eq!(Segment::try_from(over_cap), Err(SegmentError::TooLong));
}

#[test]
fn bytes_outside_the_allowlist_are_refused() {
    let refused = [
        "a/b",
        "a\\b",
        "a b",
        "a\0b",
        "a\nb",
        "a?b",
        "a#b",
        "a+b",
        "a@b",
        // Characters that look like `.` and `/`, refused before any normalization could fold them.
        "\u{2024}\u{2024}",
        "\u{FF0E}",
        "a\u{2215}b",
        "a\u{FF0F}b",
        "é",
    ];
    for piece in refused {
        assert_eq!(
            Segment::try_from(piece),
            Err(SegmentError::Disallowed),
            "{piece:?}"
        );
    }
}

#[test]
fn from_str_is_the_same_rule() {
    let parsed: Result<Segment, _> = "..".parse();
    assert_eq!(parsed, Err(SegmentError::DotSegment));
}
