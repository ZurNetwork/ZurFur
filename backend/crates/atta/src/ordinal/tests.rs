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
