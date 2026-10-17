use super::*;

/// A number the tests expect.
fn ordinal(number: u32) -> Option<NameOrdinal> {
    let ordinal = NameOrdinal::try_from(number).expect("a valid test ordinal");
    Some(ordinal)
}

/// The numbers for `names` in listing order, each placed by its position in the list.
fn numbers(names: &[&str]) -> Vec<Option<NameOrdinal>> {
    let entries: Vec<(&str, usize)> = names
        .iter()
        .enumerate()
        .map(|(position, name)| (*name, position))
        .collect();
    ordinals(&entries)
}

#[test]
fn zero_and_one_are_not_ordinals() {
    assert_eq!(NameOrdinal::try_from(0), Err(NameOrdinalError));
    assert_eq!(NameOrdinal::try_from(1), Err(NameOrdinalError));
}

#[test]
fn two_and_up_are_ordinals_that_read_back_unchanged() {
    for number in [2, 3, 1000, u32::MAX] {
        let ordinal = NameOrdinal::try_from(number).expect("an ordinal");

        assert_eq!(u32::from(ordinal), number);
        assert_eq!(ordinal.to_string(), number.to_string());
    }
}

#[test]
fn names_that_all_differ_get_no_number() {
    let numbered = numbers(&["Alpha", "Beta", "Gamma"]);

    assert_eq!(numbered, [None, None, None]);
}

#[test]
fn the_first_of_a_group_is_plain_and_the_later_ones_rise_from_two() {
    let numbered = numbers(&["Test 1", "Test 1", "Test 1"]);

    let expected = [None, ordinal(2), ordinal(3)];
    assert_eq!(numbered, expected);
}

#[test]
fn a_number_skips_a_name_another_entry_already_shows() {
    let numbered = numbers(&["Test 1", "Test 1", "Test 1 (2)"]);

    let expected = [None, ordinal(3), None];
    assert_eq!(numbered, expected);
}

#[test]
fn numbers_keep_rising_past_a_skipped_name() {
    let numbered = numbers(&["Test 1", "Test 1", "Test 1 (3)", "Test 1"]);

    let expected = [None, ordinal(2), None, ordinal(4)];
    assert_eq!(numbered, expected);
}

#[test]
fn a_group_spans_the_whole_listing_not_just_neighbours() {
    let numbered = numbers(&["Test 1", "Other", "Test 1"]);

    let expected = [None, None, ordinal(2)];
    assert_eq!(numbered, expected);
}

#[test]
fn names_differing_only_in_case_get_no_number() {
    let numbered = numbers(&["Test 1", "test 1", "TEST 1"]);

    assert_eq!(numbered, [None, None, None]);
}

#[test]
fn the_group_is_numbered_in_its_tiebreak_order_not_its_position() {
    let entries = [("Twin", 9), ("Twin", 3), ("Twin", 5)];

    let numbered = ordinals(&entries);

    let expected = [ordinal(3), None, ordinal(2)];
    assert_eq!(numbered, expected);
}

#[test]
fn the_key_is_the_exact_name() {
    assert_eq!(comparison_key("Test 1"), "Test 1");
    assert_ne!(comparison_key("Test 1"), comparison_key("test 1"));
}

/// The case file the server's rule and the web's copy of it both answer to, from this crate's
/// manifest directory.
const SHARED_CASES: &str = "../../../contract/golden/name-ordinals.json";

/// One entry of a shared case: its name, whether the viewer sees it, and the number it is
/// expected to be shown with.
struct SharedEntry {
    name: String,
    visible: bool,
    ordinal: Option<NameOrdinal>,
}

/// One shared case: what it shows, and its entries in listing order.
struct SharedCase {
    description: String,
    entries: Vec<SharedEntry>,
}

/// A shared entry read from its JSON object.
fn shared_entry(value: &serde_json::Value) -> SharedEntry {
    let name = value["name"].as_str().expect("an entry's name");
    let visible = value["visible"].as_bool().expect("an entry's visibility");
    let ordinal = &value["ordinal"];
    let ordinal = (!ordinal.is_null()).then(|| {
        let number = ordinal.as_u64().expect("an ordinal is a number or null");
        let number = u32::try_from(number).expect("an ordinal fits a u32");
        NameOrdinal::try_from(number).expect("an ordinal is 2 or more")
    });
    SharedEntry {
        name: name.to_owned(),
        visible,
        ordinal,
    }
}

/// A shared case read from its JSON object.
fn shared_case(value: &serde_json::Value) -> SharedCase {
    let description = value["description"].as_str().expect("a case's description");
    let entries = value["entries"].as_array().expect("a case's entries");
    SharedCase {
        description: description.to_owned(),
        entries: entries.iter().map(shared_entry).collect(),
    }
}

/// Every case in the shared case file.
fn shared_cases() -> Vec<SharedCase> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SHARED_CASES);
    let text = std::fs::read_to_string(&path).expect("reads the shared case file");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the case file is JSON");
    let cases = document["cases"].as_array().expect("a list of cases");
    cases.iter().map(shared_case).collect()
}

#[test]
fn the_shared_cases_hold() {
    let cases = shared_cases();
    assert!(!cases.is_empty(), "the shared case file holds cases");

    for case in cases {
        let visible: Vec<&SharedEntry> =
            case.entries.iter().filter(|entry| entry.visible).collect();
        let named: Vec<(&str, usize)> = visible
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.name.as_str(), position))
            .collect();

        let numbered = ordinals(&named);

        let expected: Vec<Option<NameOrdinal>> =
            visible.iter().map(|entry| entry.ordinal).collect();
        assert_eq!(numbered, expected, "{}", case.description);
        for hidden in case.entries.iter().filter(|entry| !entry.visible) {
            assert_eq!(
                hidden.ordinal, None,
                "{}: a hidden entry is never numbered",
                case.description
            );
        }
    }
}
