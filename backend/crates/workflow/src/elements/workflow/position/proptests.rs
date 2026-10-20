use super::*;
use proptest::prelude::*;

/// A well-formed key: at least one digit, last digit non-zero.
fn key() -> impl Strategy<Value = Position> {
    "[0-9A-Za-z]{0,8}[1-9A-Za-z]"
        .prop_map(|s| s.parse().expect("the regex guarantees the invariant"))
}

proptest! {
    #[test]
    fn between_lands_strictly_inside_any_ordered_pair(a in key(), b in key()) {
        prop_assume!(a != b);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mid = Position::between(Some(&lo), Some(&hi));
        prop_assert!(lo < mid);
        prop_assert!(mid < hi);
        prop_assert!(!mid.as_ref().ends_with('0'));
        prop_assert_eq!(mid.as_ref().parse::<Position>(), Ok(mid.clone()));
    }

    #[test]
    fn open_ends_bracket_any_key(k in key()) {
        let before = Position::between(None, Some(&k));
        let after = Position::between(Some(&k), None);
        prop_assert!(before < k);
        prop_assert!(k < after);
        prop_assert_eq!(before.as_ref().parse::<Position>(), Ok(before.clone()));
        prop_assert_eq!(after.as_ref().parse::<Position>(), Ok(after.clone()));
    }

    #[test]
    fn display_then_parse_is_identity(s in "[0-9A-Za-z]{0,16}[1-9A-Za-z]") {
        let parsed = s.parse::<Position>();
        prop_assert!(parsed.is_ok());
        let p = parsed.expect("checked above");
        prop_assert_eq!(p.to_string(), s.clone());
        prop_assert_eq!(p.as_ref(), s.as_str());
    }

    #[test]
    fn parse_accepts_exactly_the_invariant(s in prop_oneof!["[ -~]{0,12}", "\\PC{0,6}"]) {
        let expected = !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_alphanumeric())
            && !s.ends_with('0');
        prop_assert_eq!(s.parse::<Position>().is_ok(), expected);
    }

    #[test]
    fn random_inserts_and_deletes_keep_the_order(
        ops in prop::collection::vec((any::<u16>(), any::<bool>()), 1..80)
    ) {
        let mut keys = vec![Position::between(None, None)];
        for (r, del) in ops {
            let i = (r as usize) % (keys.len() + 1);
            let lo = i.checked_sub(1).map(|j| &keys[j]);
            let key = Position::between(lo, keys.get(i));
            keys.insert(i, key);
            if del && keys.len() > 1 {
                let j = (r as usize) % keys.len();
                keys.remove(j);
            }
            prop_assert!(keys.windows(2).all(|w| w[0] < w[1]));
            for k in &keys {
                prop_assert_eq!(k.as_ref().parse::<Position>(), Ok(k.clone()));
            }
        }
    }

    #[test]
    #[ignore = "midpoint's prefix guard is not built yet"]
    fn midpoint_never_stops_at_a_proper_prefix_of_hi(a in key(), b in key()) {
        prop_assume!(a != b);
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mid = Position::between(Some(&lo), Some(&hi));
        prop_assert!(!hi.as_ref().starts_with(mid.as_ref()));
    }
}
