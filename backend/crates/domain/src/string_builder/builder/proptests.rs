use super::*;
use proptest::prelude::*;

/// A short run of whitespace characters, including non-ASCII ones
/// (`\u{a0}` NBSP, `\u{3000}` ideographic space) that `char::is_whitespace`
/// still counts.
fn ws() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![' ', '\t', '\n', '\r', '\u{a0}', '\u{3000}']),
        1..6,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #[test]
    fn trimmed_is_std_trim_and_idempotent(s in any::<String>()) {
        let once = StringBuilder::new(s.clone()).trimmed().build();
        let twice = StringBuilder::new(s.clone()).trimmed().trimmed().build();
        prop_assert_eq!(once.clone(), Ok(s.trim().to_owned()));
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn non_empty_is_order_sensitive(w in ws()) {
        let trim_then_reject = StringBuilder::new(w.clone()).trimmed().non_empty().build();
        prop_assert_eq!(trim_then_reject, Err(StringBuilderViolation::Empty));

        let reject_then_trim = StringBuilder::new(w).non_empty().trimmed().build();
        prop_assert_eq!(reject_then_trim, Ok(String::new()));
    }

    #[test]
    fn non_empty_from_is_the_two_rule_chain(s in any::<String>()) {
        let via_helper = StringBuilder::non_empty_from(s.clone()).build();
        let via_chain = StringBuilder::new(s).trimmed().non_empty().build();
        prop_assert_eq!(via_helper, via_chain);
    }

    #[test]
    fn max_chars_counts_chars(s in any::<String>(), n in 0..40usize) {
        let len = s.chars().count();
        let result = StringBuilder::new(s.clone()).max_chars(n).build();
        let expected = if len > n {
            Err(StringBuilderViolation::TooLong { max: n, len })
        } else {
            Ok(s)
        };
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn control_rules(
        s in any::<String>(),
        allowed in prop::collection::vec(
            prop::sample::select(vec!['\n', '\t', '\r', '\0']),
            0..3,
        )
    ) {
        let no_control_ok = StringBuilder::new(s.clone()).no_control().build().is_ok();
        let expected_no_control_ok = !s.chars().any(|c| c.is_control());
        prop_assert_eq!(no_control_ok, expected_no_control_ok);

        let except_ok = StringBuilder::new(s.clone())
            .no_control_except(&allowed)
            .build()
            .is_ok();
        let expected_except_ok = s
            .chars()
            .filter(|c| c.is_control())
            .all(|c| allowed.contains(&c));
        prop_assert_eq!(except_ok, expected_except_ok);

        let except_empty = StringBuilder::new(s.clone()).no_control_except(&[]).build();
        let plain_no_control = StringBuilder::new(s).no_control().build();
        prop_assert_eq!(except_empty, plain_no_control);
    }
}
