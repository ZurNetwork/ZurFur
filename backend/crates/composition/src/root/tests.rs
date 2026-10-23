use super::*;

/// `text`, wiped on drop, as the environment read hands it over.
fn encoded(text: &str) -> Zeroizing<String> {
    Zeroizing::new(text.to_owned())
}

#[test]
fn a_32_byte_base64_value_decodes_trimmed() {
    let value = encoded("  q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6s=\n");

    let root = RootSecret::try_from(value).expect("32 bytes of base64");

    let bytes = root.into_bytes();
    assert_eq!(*bytes, [0xABu8; 32]);
}

#[test]
fn a_value_that_is_not_32_bytes_is_refused_by_the_rule_only() {
    let short = encoded("q6urq6urq6urq6urq6urqw==");
    let long = encoded("q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6s=");
    let empty = encoded("   ");

    for value in [short, long, empty] {
        let refusal = RootSecret::try_from(value).expect_err("only 32 bytes are a root");

        assert_eq!(refusal, RootSecretError::NotThirtyTwoBytes);
        assert_eq!(
            refusal.to_string(),
            "ZURFUR_DID_KEY_ROOT_KEY is not 32 bytes"
        );
    }
}

#[test]
fn bad_base64_symbol_names_no_character() {
    // An invalid byte in the middle, and a last symbol whose trailing bits aren't zero:
    // base64's own error prints the offending character in both.
    let bad_middle_symbol = "q6urq6urq6urq6urq6ur#6urq6urq6urq6urq6urq6s=";
    let bad_last_symbol = "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6v=";
    let cases = [(bad_middle_symbol, '#'), (bad_last_symbol, 'v')];

    for (value, offending) in cases {
        let refusal = RootSecret::try_from(encoded(value)).expect_err("not base64");

        let message = refusal.to_string();
        assert_eq!(message, "ZURFUR_DID_KEY_ROOT_KEY is not base64");
        assert!(!message.contains(offending), "{message}");
        let shown = format!("{refusal:?}");
        assert!(!shown.contains(offending), "{shown}");
    }
}

#[test]
fn an_unset_root_names_the_variable() {
    let message = RootSecretError::Unset.to_string();

    assert_eq!(message, "ZURFUR_DID_KEY_ROOT_KEY is not set");
}

#[test]
fn debug_prints_no_root_bytes() {
    let root = RootSecret::from(Zeroizing::new([0xABu8; 32]));

    let shown = format!("{root:?}");

    assert_eq!(shown, "RootSecret { .. }");
    let root_forms = ["ab", "AB", "q6ur", "171"];
    for form in root_forms {
        assert!(!shown.contains(form), "{shown} holds {form}");
    }
}
