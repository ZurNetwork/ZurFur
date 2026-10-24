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

// Each commission rule's refusal a user act can reach answers its own 409; the system's skip
// is a store failure.
#[test]
fn each_rule_refusal_answers_its_own_409() {
    use domain::elements::commission::CommissionRefusal;

    let answered: Vec<(&'static str, i32, String)> = [
        CommissionRefusal::LifecycleMoveRefused,
        CommissionRefusal::CommissionDisputed,
        CommissionRefusal::NoDeadline,
        CommissionRefusal::CommissionLate,
        CommissionRefusal::NothingOwed,
    ]
    .into_iter()
    .map(|refusal| {
        let token: &'static str = refusal.into();
        let problem = Problem::from(CommissionError::from(refusal));
        (token, problem.status, problem.code)
    })
    .collect();

    let expected = [
        (
            "lifecycle_move_refused",
            409,
            "lifecycle_move_refused".to_owned(),
        ),
        ("commission_disputed", 409, "commission_disputed".to_owned()),
        ("no_deadline", 409, "no_deadline".to_owned()),
        ("commission_late", 409, "commission_late".to_owned()),
        ("nothing_owed", 500, "internal_error".to_owned()),
    ];
    assert_eq!(answered, expected);
}
