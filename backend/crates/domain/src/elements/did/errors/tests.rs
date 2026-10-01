use crate::elements::did::Did;

#[test]
fn the_error_names_the_input_and_nothing_else() {
    let error = "nope".parse::<Did>().unwrap_err();
    assert_eq!(
        error.to_string(),
        "not a DID (expected `did:<method>:<id>`): \"nope\""
    );
}
