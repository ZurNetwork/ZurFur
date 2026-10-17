use super::super::Handle;
use super::*;
use proptest::prelude::*;

/// A single label: charset-valid, no hyphen edges; `xn--` labels included.
fn label() -> impl Strategy<Value = String> {
    "[a-z0-9]([a-z0-9-]{0,10}[a-z0-9])?"
}

/// A top-level label that does not start with a digit and is not disallowed.
fn tld() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,8}".prop_filter("not a disallowed TLD", |tld| {
        !DISALLOWED_TLDS.contains(&tld.as_str())
    })
}

/// A valid, already-normalized handle.
fn valid() -> impl Strategy<Value = String> {
    (prop::collection::vec(label(), 1..4), tld()).prop_map(|(labels, tld)| {
        let mut parts = labels;
        parts.push(tld);
        parts.join(".")
    })
}

/// A normalized handle paired with a raw spelling of it: possibly uppercase.
fn raw_of_valid() -> impl Strategy<Value = (String, String)> {
    (valid(), any::<bool>()).prop_map(|(canonical, upper)| {
        let raw = if upper {
            canonical.to_uppercase()
        } else {
            canonical.clone()
        };
        (canonical, raw)
    })
}

/// Whitespace a person might leave around a handle: ASCII and Unicode spaces,
/// tabs and line breaks.
fn whitespace() -> impl Strategy<Value = char> {
    prop::sample::select(vec![
        ' ', '\t', '\n', '\r', '\u{A0}', '\u{2003}', '\u{3000}',
    ])
}

proptest! {
    #[test]
    fn accepts_the_valid_region((canonical, raw) in raw_of_valid()) {
        let parsed = raw.parse::<AtHandle>().map(|handle| handle.to_string());
        prop_assert_eq!(parsed, Ok(canonical));
    }

    #[test]
    fn refuses_surrounding_whitespace(
        canonical in valid(),
        pad in whitespace(),
        leading in any::<bool>(),
    ) {
        let padded = if leading {
            format!("{pad}{canonical}")
        } else {
            format!("{canonical}{pad}")
        };
        prop_assert_eq!(padded.parse::<AtHandle>(), Err(HandleError::InvalidChar(pad)));
    }

    #[test]
    fn refuses_every_trailing_dot(canonical in valid()) {
        let with_trailing_dot = format!("{canonical}.");
        let parsed = with_trailing_dot.parse::<AtHandle>();
        prop_assert_eq!(parsed, Err(HandleError::EmptySegment));
    }

    #[test]
    fn normalization_is_idempotent(
        raw in prop_oneof![raw_of_valid().prop_map(|(_, raw)| raw), any::<String>()]
    ) {
        if let Ok(handle) = raw.parse::<AtHandle>() {
            prop_assert_eq!(handle.to_string().parse::<AtHandle>(), Ok(handle));
        }
    }

    #[test]
    fn every_claimed_handle_is_an_at_handle(
        raw in prop_oneof![raw_of_valid().prop_map(|(_, raw)| raw), any::<String>()]
    ) {
        if let Ok(claimed) = raw.parse::<Handle>() {
            let parsed = claimed.to_string().parse::<AtHandle>().map(|handle| handle.to_string());
            prop_assert_eq!(parsed, Ok(claimed.to_string()));
        }
    }
}
