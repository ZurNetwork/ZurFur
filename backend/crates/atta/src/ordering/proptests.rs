use std::cmp::Ordering;

use proptest::prelude::*;

use super::*;

/// Names built from the characters that stress the order: both cases, digits and leading
/// zeros, punctuation, a space, and letters whose lowercase differs in length or form.
fn name() -> impl Strategy<Value = String> {
    let characters = prop::sample::select(vec![
        'a', 'A', 'b', 'B', 'z', '0', '1', '2', '9', '-', '.', ' ', 'é', 'É', 'ß', 'Σ', 'σ', 'ς',
        '２', 'İ',
    ]);
    prop::collection::vec(characters, 0..8).prop_map(|characters| characters.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn the_order_is_antisymmetric_and_equal_only_for_identical_names(left in name(), right in name()) {
        prop_assert_eq!(name_order(&left, &right), name_order(&right, &left).reverse());
        prop_assert_eq!(name_order(&left, &right) == Ordering::Equal, left == right);
    }

    #[test]
    fn the_order_is_transitive(first in name(), second in name(), third in name()) {
        let mut names = [first, second, third];
        names.sort_by(|left, right| name_order(left, right));
        prop_assert_ne!(name_order(&names[0], &names[1]), Ordering::Greater);
        prop_assert_ne!(name_order(&names[1], &names[2]), Ordering::Greater);
        prop_assert_ne!(name_order(&names[0], &names[2]), Ordering::Greater);
    }

    #[test]
    fn sorting_doesnt_depend_on_the_input_order(names in prop::collection::vec(name(), 0..40)) {
        let mut forward = names.clone();
        forward.sort_by(|left, right| name_order(left, right));
        let mut backward: Vec<String> = names.into_iter().rev().collect();
        backward.sort_by(|left, right| name_order(left, right));
        prop_assert_eq!(forward, backward);
    }
}
