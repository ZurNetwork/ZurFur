use super::*;
use domain::elements::handle::HandleError;

// A handle the configured namespace withholds is the caller's mistake: a 422
// carrying the rule's message, never a 500.
#[test]
fn a_withheld_handle_is_an_invalid_request() {
    let refusals = [
        HandleError::ReservedLabel("admin".into()),
        HandleError::NamespaceApex,
    ];
    for rule in refusals {
        let message = rule.to_string();
        let error = CommissionError::CommonError(CommonError::InvalidHandle(rule));
        let problem = Problem::from(error);
        assert_eq!(problem.code, "invalid_request");
        assert_eq!(problem.status, 422);
        assert_eq!(problem.detail, message);
    }
}
