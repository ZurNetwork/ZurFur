use super::super::reserved::{RESERVED_LABELS, RESERVED_TLDS};
use super::*;
use proptest::prelude::*;

/// A single dot-separated label: charset-valid, no hyphen edges.
fn label() -> impl Strategy<Value = String> {
    "[a-z0-9]([a-z0-9-]{0,10}[a-z0-9])?"
        .prop_filter("no punycode label", |s| !s.starts_with("xn--"))
}

/// A top-level segment that isn't one of the reserved TLDs.
fn tld() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{1,8}".prop_filter("not a reserved TLD", |s| {
        !RESERVED_TLDS.contains(&s.as_str())
    })
}

/// A fully-valid, already-normalized handle string.
fn valid() -> impl Strategy<Value = String> {
    (prop::collection::vec(label(), 1..4), tld())
        .prop_map(|(labels, tld)| {
            let mut parts = labels;
            parts.push(tld);
            parts.join(".")
        })
        .prop_filter(
            "leftmost label must not be reserved in the zurfur namespace",
            |handle| {
                if handle == "zurfur.app" || handle.ends_with(".zurfur.app") {
                    let leftmost = handle.split('.').next().unwrap_or("");
                    !RESERVED_LABELS.contains(&leftmost)
                } else {
                    true
                }
            },
        )
}

/// Pairs a normalized handle with a raw variant of it that should normalize
/// back to the same thing: surrounding whitespace, uppercasing, a trailing dot.
fn raw_of_valid() -> impl Strategy<Value = (String, String)> {
    (
        valid(),
        "[ \t]{0,2}",
        any::<bool>(),
        any::<bool>(),
        "[ \t]{0,2}",
    )
        .prop_map(|(canon, leading, upper, trailing_dot, trailing)| {
            let body = if upper {
                canon.to_uppercase()
            } else {
                canon.clone()
            };
            let dot = if trailing_dot { "." } else { "" };
            let raw = format!("{leading}{body}{dot}{trailing}");
            (canon, raw)
        })
}

proptest! {
    #[test]
    fn gate_accepts_the_valid_region((canon, raw) in raw_of_valid()) {
        let parsed = raw.parse::<Handle>().map(|h| h.as_ref().to_owned());
        prop_assert_eq!(parsed, Ok(canon));
    }

    #[test]
    fn normalization_is_idempotent(
        raw in prop_oneof![raw_of_valid().prop_map(|(_, r)| r), any::<String>()]
    ) {
        if let Ok(h) = raw.parse::<Handle>() {
            prop_assert_eq!(h.to_string().parse::<Handle>(), Ok(h));
        }
    }
}
