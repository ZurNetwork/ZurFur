use super::*;

#[test]
fn disallowed_tlds_are_the_handle_specs_eight() {
    let spec_list = [
        "alt",
        "arpa",
        "example",
        "internal",
        "invalid",
        "local",
        "localhost",
        "onion",
    ];
    assert_eq!(DISALLOWED_TLDS, spec_list);
}

#[test]
fn reserved_tlds_are_the_disallowed_tlds_plus_test() {
    let spec_list_plus_test = [
        "alt",
        "arpa",
        "example",
        "internal",
        "invalid",
        "local",
        "localhost",
        "onion",
        "test",
    ];
    assert_eq!(RESERVED_TLDS, spec_list_plus_test);
}
