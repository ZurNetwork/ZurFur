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
    let mut disallowed_plus_test = DISALLOWED_TLDS.to_vec();
    disallowed_plus_test.push("test");
    assert_eq!(RESERVED_TLDS, disallowed_plus_test);
}
