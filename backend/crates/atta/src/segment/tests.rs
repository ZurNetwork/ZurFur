use super::*;

#[test]
fn keys_the_den_uses_parse() {
    let keys = [
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

/// The case file this rule, kernel's node segment rule and the store's check all answer to,
/// from this crate's manifest directory.
const SHARED_CASES: &str = "../../../contract/golden/segments.json";

/// One shared case: what it shows, and its segment text.
struct SharedCase {
    description: String,
    segment: String,
}

/// A shared case read from its JSON object; a `repeat` case is its text `times` times over.
fn shared_case(value: &serde_json::Value) -> SharedCase {
    let description = value["description"].as_str().expect("a case's description");
    let segment = match value["segment"].as_str() {
        Some(segment) => segment.to_owned(),
        None => {
            let text = value["repeat"].as_str().expect("a segment or a repeat");
            let times = value["times"].as_u64().expect("a repeat's times");
            let times = usize::try_from(times).expect("the times fit a usize");
            text.repeat(times)
        }
    };
    SharedCase {
        description: description.to_owned(),
        segment,
    }
}

/// The accepted and the refused cases of the shared case file.
fn shared_cases() -> (Vec<SharedCase>, Vec<SharedCase>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SHARED_CASES);
    let text = std::fs::read_to_string(&path).expect("reads the shared case file");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the case file is JSON");
    let read = |list: &str| -> Vec<SharedCase> {
        document[list]
            .as_array()
            .expect("a list of cases")
            .iter()
            .map(shared_case)
            .collect()
    };
    (read("accepted"), read("refused"))
}

#[test]
fn segments_follow_the_shared_cases() {
    let (accepted, refused) = shared_cases();
    let holds =
        |cases: &[SharedCase], segment: &str| cases.iter().any(|case| case.segment == segment);
    let longest = "a".repeat(MAX_SEGMENT_BYTES);
    let one_too_long = "a".repeat(MAX_SEGMENT_BYTES + 1);
    assert!(holds(&accepted, "a_b"), "the underscore case is present");
    assert!(holds(&accepted, &longest), "the 2048-byte case is present");
    assert!(holds(&refused, "\u{e9}"), "the accent case is present");
    assert!(holds(&refused, ".."), "the dot-dot case is present");
    assert!(
        holds(&refused, &one_too_long),
        "the 2049-byte case is present"
    );

    for case in &accepted {
        let parsed = Segment::try_from(case.segment.as_str());
        assert_eq!(
            parsed.map(|segment| segment.to_string()),
            Ok(case.segment.clone()),
            "{}",
            case.description
        );
    }
    for case in &refused {
        let parsed = Segment::try_from(case.segment.as_str());
        assert!(parsed.is_err(), "{}", case.description);
    }
}
