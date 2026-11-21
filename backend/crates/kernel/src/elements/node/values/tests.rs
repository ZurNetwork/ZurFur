use super::*;

/// The case file Atta's segment rule, this one and the store's check all answer to, from this
/// crate's manifest directory.
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

/// Whether `cases` holds a case for `segment`.
fn holds(cases: &[SharedCase], segment: &str) -> bool {
    cases.iter().any(|case| case.segment == segment)
}

#[test]
fn segments_follow_the_shared_cases() {
    let (accepted, refused) = shared_cases();
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
        let parsed = NodeSegment::try_from(case.segment.as_str());
        assert_eq!(
            parsed.map(|segment| segment.to_string()),
            Ok(case.segment.clone()),
            "{}",
            case.description
        );
    }
    for case in &refused {
        let parsed = NodeSegment::try_from(case.segment.as_str());
        assert!(parsed.is_err(), "{}", case.description);
    }
}

#[test]
fn the_shared_did_case_is_fixture_style() {
    let (accepted, _refused) = shared_cases();

    let dids: Vec<&SharedCase> = accepted
        .iter()
        .filter(|case| case.segment.starts_with("did:"))
        .collect();

    assert!(!dids.is_empty(), "the shared cases hold a DID");
    for case in dids {
        assert!(
            case.segment.contains("fixture"),
            "a DID case is made up and says so: {}",
            case.description
        );
    }
}

#[test]
fn each_refusal_names_its_rule() {
    let longest = "a".repeat(MAX_SEGMENT_BYTES + 1);
    let cases = [
        ("", NodeSegmentError::Empty),
        (longest.as_str(), NodeSegmentError::TooLong),
        (".", NodeSegmentError::DotSegment),
        ("..", NodeSegmentError::DotSegment),
        ("a/b", NodeSegmentError::Disallowed),
    ];

    for (text, expected) in cases {
        assert_eq!(NodeSegment::try_from(text), Err(expected), "{text:?}");
    }
}

#[test]
fn from_str_and_both_conversions_are_one_rule() {
    let from_str: Result<NodeSegment, _> = "a~b".parse();
    let from_string = NodeSegment::try_from("a~b".to_owned());
    let from_slice = NodeSegment::try_from("a~b");

    assert_eq!(from_str, from_slice);
    assert_eq!(from_string, from_slice);
}

#[test]
fn segments_order_bytewise() {
    let mut segments: Vec<NodeSegment> = ["b", "a", "B", "_", "0", "~", "A"]
        .into_iter()
        .map(|text| NodeSegment::try_from(text).expect("a segment"))
        .collect();

    segments.sort();

    let ordered: Vec<&str> = segments.iter().map(AsRef::as_ref).collect();
    assert_eq!(ordered, ["0", "A", "B", "_", "a", "b", "~"]);
}
