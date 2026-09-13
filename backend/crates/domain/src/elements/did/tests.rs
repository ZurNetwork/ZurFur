use super::*;

#[test]
fn well_formed_dids_parse() {
    for text in [
        "did:plc:ewvi7nxzyoun6zhxrhs64oiz",
        "did:web:example.com",
        "did:web:example.com:user:alice",
        "did:key:z6Mk",
    ] {
        let did: Did = text.parse().unwrap();
        assert_eq!(did.to_string(), text);
    }
}

#[test]
fn malformed_dids_are_refused() {
    for text in [
        "",
        "did:",
        "did:plc:",
        "plc:abc",
        "did:PLC:abc",
        "did:plc:a b",
        "DID:plc:abc",
        "did::abc",
        "'; DROP TABLE users; --",
    ] {
        assert!(text.parse::<Did>().is_err(), "{text:?} must not parse");
    }
}

#[test]
fn the_error_names_the_input_and_nothing_else() {
    let error = "nope".parse::<Did>().unwrap_err();
    assert_eq!(
        error.to_string(),
        "not a DID (expected `did:<method>:<id>`): \"nope\""
    );
}
