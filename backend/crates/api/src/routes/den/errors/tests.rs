use super::*;

#[test]
fn an_unknown_parameter_is_the_fixed_400() {
    assert_eq!(
        Problem::from(DenParamsError::Unknown),
        Problem::unknown_parameter()
    );
}

#[test]
fn a_repeated_or_unusable_parameter_is_the_fixed_422() {
    assert_eq!(
        Problem::from(DenParamsError::Repeated),
        Problem::invalid_query()
    );
    assert_eq!(
        Problem::from(DenParamsError::Malformed),
        Problem::invalid_query()
    );
}

#[test]
fn a_path_that_does_not_decode_is_the_one_not_found() {
    assert_eq!(Problem::from(NotDenPieces), Problem::node_not_found());
}

#[test]
fn every_den_miss_is_the_one_not_found() {
    assert_eq!(Problem::from(DenError::NotFound), Problem::node_not_found());
}

#[test]
fn a_store_failure_is_the_house_500_and_never_renders_its_cause() {
    let cause = anyhow::anyhow!("/user/did:plc:secret/accounts failed");
    let house_500 = Problem::from(anyhow::anyhow!("anything else"));

    let problem = Problem::from(DenError::Store(cause));

    assert_eq!(problem, house_500);
    assert_eq!(problem.status, 500);
    assert!(!problem.detail.contains("secret"));
}

#[test]
fn a_read_problem_is_the_same_problem_its_error_maps_to() {
    let cause = anyhow::anyhow!("/user/did:plc:secret failed");

    assert_eq!(
        read_problem(DenError::Store(cause), "/den"),
        Problem::from(anyhow::anyhow!("anything else"))
    );
    assert_eq!(
        read_problem(DenError::NotFound, "/den/{*path}"),
        Problem::node_not_found()
    );
}
