use super::*;

fn position(key: &str) -> Position {
    key.parse().expect("test keys hold the invariant")
}

/// Mints the key between two optional neighbours and checks the contract
/// every caller relies on: strictly between, and never ending in `0`.
fn key_between(lo: Option<&str>, hi: Option<&str>) -> String {
    let lo = lo.map(position);
    let hi = hi.map(position);
    let minted = Position::between(lo.as_ref(), hi.as_ref());
    if let Some(lo) = &lo {
        assert!(lo < &minted, "{lo} must sort before {minted}");
    }
    if let Some(hi) = &hi {
        assert!(&minted < hi, "{minted} must sort before {hi}");
    }
    assert!(!minted.as_ref().ends_with('0'), "{minted} ends in '0'");
    minted.to_string()
}

// ---- between -----------------------------------------------------------

#[test]
fn the_first_key_is_the_middle_digit() {
    assert_eq!(key_between(None, None), "V");
}

#[test]
fn before_and_after_pick_the_middle_of_the_open_side() {
    assert_eq!(key_between(None, Some("V")), "G");
    assert_eq!(key_between(Some("V"), None), "l");
}

#[test]
fn a_gap_between_digits_takes_the_middle_digit() {
    assert_eq!(key_between(Some("a"), Some("c")), "b");
    assert_eq!(key_between(Some("a"), Some("a5")), "a3");
}

#[test]
fn adjacent_digits_extend_the_lower_key() {
    assert_eq!(key_between(Some("a"), Some("b")), "aV");
    assert_eq!(key_between(Some("aV"), Some("b")), "al");
    assert_eq!(key_between(Some("a9"), Some("b")), "aa");
}

#[test]
fn the_last_digit_still_has_room_after_it() {
    assert_eq!(key_between(Some("z"), None), "zV");
}

#[test]
fn a_leading_zero_is_allowed_only_a_trailing_one_is_not() {
    assert_eq!(key_between(None, Some("1")), "0V");
    assert_eq!(key_between(None, Some("05")), "03");
}

#[test]
fn keys_stay_ordered_under_repeated_insertion_everywhere() {
    let mut keys = vec![position("V")];
    for _ in 0..200 {
        let front = Position::between(None, keys.first());
        keys.insert(0, front);
    }
    for _ in 0..200 {
        let back = Position::between(keys.last(), None);
        keys.push(back);
    }
    for i in 0..keys.len() - 1 {
        let middle = Position::between(Some(&keys[i]), Some(&keys[i + 1]));
        keys.insert(i + 1, middle);
    }

    let all_ascending = keys.windows(2).all(|pair| pair[0] < pair[1]);
    assert!(all_ascending);
    assert!(keys.iter().all(|key| !key.as_ref().ends_with('0')));
}

#[test]
#[should_panic(expected = "lo must sort before hi")]
fn between_refuses_misordered_neighbours() {
    let lo = position("b");
    let hi = position("a");
    Position::between(Some(&lo), Some(&hi));
}

// ---- ordering ----------------------------------------------------------

#[test]
fn ordering_is_bytewise_with_the_prefix_rule() {
    assert!(position("a") < position("aa"));
    assert!(position("aa") < position("b"));
    assert!(position("9") < position("A"));
    assert!(position("Z") < position("a"));
}

// ---- parse -------------------------------------------------------------

#[test]
fn parse_accepts_a_key_and_round_trips_it() {
    let parsed = "aV".parse::<Position>();
    assert_eq!(
        parsed.as_ref().map(ToString::to_string).as_deref(),
        Ok("aV")
    );
}

#[test]
fn parse_rejects_what_breaks_the_invariant() {
    assert_eq!("".parse::<Position>(), Err(PositionError::Empty));
    assert_eq!(
        "a-b".parse::<Position>(),
        Err(PositionError::InvalidDigit('-'))
    );
    assert_eq!(
        "é".parse::<Position>(),
        Err(PositionError::InvalidDigit('é'))
    );
    assert_eq!("a0".parse::<Position>(), Err(PositionError::TrailingZero));
}
