use super::*;

fn parse(raw: &str) -> Result<AtHandle, HandleError> {
    raw.parse::<AtHandle>()
}

// ---- The handle spec's own examples -----------------------------------

#[test]
fn accepts_the_specs_valid_examples() {
    let spec_valid = [
        "jay.bsky.social",
        "8.cn",
        "name.t--t",
        "a.co",
        "xn--notarealidn.com",
        "xn--fiqa61au8b7zsevnm8ak20mc4a87e.xn--fiqs8s",
        "xn--ls8h.test",
        "example.t",
    ];
    for raw in spec_valid {
        let parsed = parse(raw).map(|handle| handle.to_string());
        let expected = Ok(raw.to_owned());
        assert_eq!(parsed, expected, "{raw} is valid handle syntax");
    }
}

#[test]
fn lowercases_the_specs_uppercase_example() {
    let parsed = parse("XX.LCS.MIT.EDU").map(|handle| handle.to_string());
    let expected = Ok("xx.lcs.mit.edu".to_owned());
    assert_eq!(parsed, expected);
}

#[test]
fn refuses_the_specs_invalid_examples() {
    let spec_invalid = [
        ("jo@hn.test", HandleError::InvalidChar('@')),
        ("💩.test", HandleError::InvalidChar('💩')),
        ("john..test", HandleError::EmptySegment),
        ("xn--bcher-.tld", HandleError::HyphenEdge),
        ("john.0", HandleError::TldLeadingDigit),
        ("cn.8", HandleError::TldLeadingDigit),
        ("www.masełkowski.pl.com", HandleError::InvalidChar('ł')),
        ("org", HandleError::TooFewSegments),
        ("name.org.", HandleError::EmptySegment),
    ];
    for (raw, expected_error) in spec_invalid {
        assert_eq!(parse(raw), Err(expected_error), "{raw} is invalid syntax");
    }
}

#[test]
fn refuses_the_specs_valid_but_disallowed_examples() {
    let onion = "2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion";
    let disallowed = [
        (onion, "onion"),
        ("laptop.local", "local"),
        ("blah.arpa", "arpa"),
    ];
    for (raw, tld) in disallowed {
        let expected_error = HandleError::ReservedTld(tld.to_owned());
        assert_eq!(parse(raw), Err(expected_error), "{raw} must not resolve");
    }
}

// ---- Disallowed TLDs ---------------------------------------------------

#[test]
fn refuses_each_disallowed_tld() {
    for tld in DISALLOWED_TLDS {
        let raw = format!("alice.{tld}");
        let expected_error = HandleError::ReservedTld((*tld).to_owned());
        assert_eq!(parse(&raw), Err(expected_error), "{raw}");
    }
}

#[test]
fn refuses_the_no_handle_marker() {
    let expected_error = HandleError::ReservedTld("invalid".to_owned());
    assert_eq!(parse("handle.invalid"), Err(expected_error));
}

#[test]
fn accepts_dot_test_which_only_a_claimed_handle_refuses() {
    let parsed = parse("alice.test").map(|handle| handle.to_string());
    let expected = Ok("alice.test".to_owned());
    assert_eq!(parsed, expected);
}

// ---- Trailing and leading dots -----------------------------------------

#[test]
fn refuses_a_trailing_dot_rather_than_stripping_it() {
    assert_eq!(parse("alice.bsky.social."), Err(HandleError::EmptySegment));
}

#[test]
fn refuses_a_leading_dot_and_a_lone_dot() {
    assert_eq!(parse(".alice.bsky.social"), Err(HandleError::EmptySegment));
    assert_eq!(parse("."), Err(HandleError::EmptySegment));
}

// ---- Normalization -----------------------------------------------------

#[test]
fn refuses_surrounding_whitespace_rather_than_trimming_it() {
    let padded = [
        (" alice.bsky.social", ' '),
        ("alice.bsky.social ", ' '),
        ("\talice.bsky.social", '\t'),
        ("alice.bsky.social\n", '\n'),
        ("alice.bsky.social\r\n", '\r'),
        ("\u{A0}alice.bsky.social", '\u{A0}'),
        ("alice.bsky.social\u{3000}", '\u{3000}'),
        (" alice.bsky.social. ", ' '),
    ];
    for (raw, whitespace) in padded {
        let expected_error = HandleError::InvalidChar(whitespace);
        assert_eq!(parse(raw), Err(expected_error), "{raw:?}");
    }
}

#[test]
fn refuses_inner_whitespace() {
    assert_eq!(
        parse("ali ce.bsky.social"),
        Err(HandleError::InvalidChar(' '))
    );
}

#[test]
fn refuses_an_empty_or_blank_input() {
    assert_eq!(parse(""), Err(HandleError::Empty));
    // Blank is not trimmed to empty: it is one label, so not a handle.
    assert_eq!(parse("   "), Err(HandleError::TooFewSegments));
}

#[test]
fn lowercases_ascii_only() {
    // U+212A KELVIN SIGN lowercases to an ASCII `k` under Unicode rules; the
    // spec lowercases ASCII only, so it stays a foreign character and fails.
    let kelvin_sign = '\u{212A}';
    let raw = format!("{kelvin_sign}.bsky.social");
    assert_eq!(parse(&raw), Err(HandleError::InvalidChar(kelvin_sign)));
}

#[test]
fn display_and_as_ref_give_the_normalized_text() {
    let handle = parse("Alice.Bsky.Social").expect("a valid handle");
    let normalized = "alice.bsky.social";
    assert_eq!(handle.to_string(), normalized);
    assert_eq!(handle.as_ref(), normalized);
}

// ---- Lengths -----------------------------------------------------------

#[test]
fn accepts_exactly_253_chars() {
    let label = "a".repeat(63);
    let tld = "a".repeat(61);
    let raw = format!("{label}.{label}.{label}.{tld}");
    assert_eq!(raw.len(), 253);
    let parsed = parse(&raw).map(|handle| handle.to_string());
    assert_eq!(parsed, Ok(raw));
}

#[test]
fn refuses_254_chars() {
    let label = "a".repeat(63);
    let tld = "a".repeat(62);
    let raw = format!("{label}.{label}.{label}.{tld}");
    assert_eq!(parse(&raw), Err(HandleError::TooLong(254)));
}

#[test]
fn accepts_a_63_char_label_and_refuses_a_64_char_one() {
    let longest = format!("{}.com", "a".repeat(63));
    assert!(parse(&longest).is_ok());
    let too_long = format!("{}.com", "a".repeat(64));
    assert_eq!(parse(&too_long), Err(HandleError::SegmentTooLong(64)));
}

// ---- Labels ------------------------------------------------------------

#[test]
fn refuses_a_hyphen_at_either_end_of_a_label() {
    assert_eq!(parse("-alice.bsky.social"), Err(HandleError::HyphenEdge));
    assert_eq!(parse("alice-.bsky.social"), Err(HandleError::HyphenEdge));
    assert_eq!(parse("alice.bsky.social-"), Err(HandleError::HyphenEdge));
}

#[test]
fn accepts_inner_hyphens_and_digit_leading_inner_labels() {
    for raw in ["al-ice.bsky.social", "1alice.bsky.social", "a.b-c.d"] {
        assert!(parse(raw).is_ok(), "{raw} is valid");
    }
}

// ---- Not Zurfur's claim rules ------------------------------------------

#[test]
fn accepts_punycode_labels_anywhere() {
    for raw in ["xn--e1awd7f.com", "good.xn--abc.com", "XN--abc.com"] {
        assert!(parse(raw).is_ok(), "{raw} is a foreign IDN identity");
    }
}

#[test]
fn accepts_names_zurfur_reserves_for_its_own_claims() {
    for raw in ["api.zurfur.app", "zurfur.app", "admin.zurfur.app"] {
        assert!(parse(raw).is_ok(), "{raw} is valid syntax");
    }
}

// ---- Not handles at all ------------------------------------------------

#[test]
fn refuses_urls_dids_ips_and_at_prefixes() {
    let not_handles = [
        "https://evil.example.com",
        "http://alice.bsky.social",
        "alice.bsky.social/path",
        "alice.bsky.social:443",
        "did:plc:z72i7hdynmk6r22z27h6tvur",
        "did:web:alice.example.com",
        "127.0.0.1",
        "[::1]",
        "@alice.bsky.social",
        "alice@bsky.social",
    ];
    for raw in not_handles {
        assert!(parse(raw).is_err(), "{raw} is not a handle");
    }
}
