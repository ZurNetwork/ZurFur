use std::cmp::Ordering;

use super::*;

fn sorted(names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    names.sort_by(|left, right| name_order(left, right));
    names
}

#[test]
fn numbers_sort_by_their_value() {
    let expected = ["file1", "file2", "file10", "file20", "file100"];
    assert_eq!(
        sorted(&["file10", "file2", "file100", "file1", "file20"]),
        expected
    );
}

#[test]
fn case_doesnt_decide_the_order() {
    let expected = ["apple", "Banana", "cherry", "Zebra"];
    assert_eq!(sorted(&["Zebra", "cherry", "apple", "Banana"]), expected);
}

#[test]
fn names_equal_but_for_case_order_by_the_name_itself() {
    assert_eq!(name_order("File", "file"), Ordering::Less);
    assert_eq!(name_order("file", "File"), Ordering::Greater);
}

#[test]
fn leading_zeros_compare_equal_in_value_then_by_the_name_itself() {
    assert_eq!(name_order("file02", "file2"), Ordering::Less);
    assert_eq!(name_order("file2", "file02"), Ordering::Greater);
    assert_eq!(name_order("file02", "file3"), Ordering::Less);
    assert_eq!(name_order("file002", "file10"), Ordering::Less);
}

#[test]
fn a_number_sorts_before_letters_and_after_punctuation_as_its_digit_does() {
    let expected = ["-dash", "1st", "a"];
    assert_eq!(sorted(&["a", "1st", "-dash"]), expected);
}

#[test]
fn a_prefix_sorts_first() {
    assert_eq!(name_order("file", "file2"), Ordering::Less);
    assert_eq!(name_order("file", "files"), Ordering::Less);
}

#[test]
fn numbers_longer_than_any_integer_still_compare_by_value() {
    let longer = format!("n{}", "9".repeat(40));
    let shorter = format!("n1{}", "0".repeat(38));
    assert_eq!(name_order(&shorter, &longer), Ordering::Less);
}

#[test]
fn only_identical_names_compare_equal() {
    assert_eq!(name_order("Old sketch", "Old sketch"), Ordering::Equal);
    assert_ne!(name_order("a1", "a01"), Ordering::Equal);
}
