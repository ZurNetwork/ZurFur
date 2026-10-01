use super::*;

// The mapping is total and the numbers are the contract.
#[test]
fn every_class_has_its_documented_code() {
    assert_eq!(ExitClass::Domain.code(), 1);
    assert_eq!(ExitClass::Usage.code(), 2);
    assert_eq!(ExitClass::Infra.code(), 3);
    assert_eq!(ExitClass::Interrupted.code(), 130);
}

#[test]
fn a_problem_renders_class_code_detail() {
    let error = CliError::domain("not_authenticated", "run `zurfur session login` first");
    let rendered = serde_json::to_string(&error.problem()).unwrap();
    assert_eq!(
        rendered,
        r#"{"class":"domain","code":"not_authenticated","detail":"run `zurfur session login` first"}"#
    );
}

#[test]
fn constructors_pick_the_class() {
    assert_eq!(CliError::domain("x", "y").class(), ExitClass::Domain);
    assert_eq!(CliError::infra("x", "y").class(), ExitClass::Infra);
    assert_eq!(CliError::interrupted().class(), ExitClass::Interrupted);
}
