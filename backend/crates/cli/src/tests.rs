use super::*;

// Only an allowlisted key's own message passes through: any other key's
// message renders the generic detail, so a value it carries never reaches
// stderr.
#[test]
fn a_message_on_a_key_outside_the_allowlist_renders_the_generic_detail() {
    let mut secret_message = figment::Error::from("expected a key, found hunter2".to_owned());
    secret_message.path = vec!["did_key_root_key".to_owned()];
    let load_error = LoadError::Figment(Box::new(secret_message));

    let detail = config_problem(&load_error).problem().detail;

    assert!(detail.contains("could not be loaded"), "{detail}");
    assert!(detail.contains("`did_key_root_key`"), "{detail}");
    assert!(!detail.contains("hunter2"), "{detail}");
}

// `public_url`'s parser names only the rule it broke, so its message is shown.
#[test]
fn a_message_on_an_allowlisted_key_is_shown() {
    let mut rule_message =
        figment::Error::from("`public_url` must be a bare origin: it carries a path".to_owned());
    rule_message.path = vec!["public_url".to_owned()];
    let load_error = LoadError::Figment(Box::new(rule_message));

    let detail = config_problem(&load_error).problem().detail;

    assert_eq!(
        detail,
        "`public_url` must be a bare origin: it carries a path"
    );
}
