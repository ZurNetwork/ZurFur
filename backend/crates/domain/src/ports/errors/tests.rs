use std::error::Error as _;

use super::*;

const ASKED_ABOUT: &str = "alice.example.com";

fn every_resolve_error() -> [ResolveError; 4] {
    let refused_cause = anyhow::anyhow!("{ASKED_ABOUT} resolved to 10.0.0.1");
    let unavailable_cause = anyhow::anyhow!("timed out reaching {ASKED_ABOUT}");
    [
        ResolveError::NotFound,
        ResolveError::NotConfirmed,
        ResolveError::Refused(refused_cause),
        ResolveError::Unavailable(unavailable_cause),
    ]
}

#[test]
fn resolve_error_messages_are_terse_and_fixed() {
    let messages = every_resolve_error().map(|error| error.to_string());
    let expected = [
        "identity not found",
        "handle not confirmed by its DID",
        "identity lookup refused",
        "identity lookup unavailable",
    ];
    assert_eq!(messages, expected);
}

#[test]
fn resolve_error_debug_names_the_variant_only() {
    let debug_forms = every_resolve_error().map(|error| format!("{error:?}"));
    let expected = ["NotFound", "NotConfirmed", "Refused", "Unavailable"];
    assert_eq!(debug_forms, expected);
}

#[test]
fn resolve_error_never_echoes_the_cause_in_any_format() {
    for error in every_resolve_error() {
        let rendered = format!("{error} {error:#} {error:?} {error:#?}");
        assert!(
            !rendered.contains(ASKED_ABOUT),
            "a format of {error} echoed its cause: {rendered}"
        );
    }
}

#[test]
fn resolve_error_carries_the_cause_as_its_source() {
    let [not_found, not_confirmed, refused, unavailable] = every_resolve_error();
    assert!(not_found.source().is_none());
    assert!(not_confirmed.source().is_none());
    let refused_source = refused.source().map(ToString::to_string);
    let expected_refused = Some(format!("{ASKED_ABOUT} resolved to 10.0.0.1"));
    assert_eq!(refused_source, expected_refused);
    let unavailable_source = unavailable.source().map(ToString::to_string);
    let expected_unavailable = Some(format!("timed out reaching {ASKED_ABOUT}"));
    assert_eq!(unavailable_source, expected_unavailable);
}
