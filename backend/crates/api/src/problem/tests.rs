use super::*;

// AC1/AC3 — a Problem serializes to exactly the five RFC 9457 members we
// promise, with our URN `type` and terse `code`.
#[test]
fn serializes_to_the_rfc9457_members() {
    let value = serde_json::to_value(Problem::already_member(
        "did:plc:abc already holds a role on account 0192.",
    ))
    .expect("serializes");

    assert_eq!(value["type"], "urn:zurfur:error:already-member");
    assert_eq!(value["code"], "already_member");
    assert_eq!(value["title"], "Already a member");
    assert_eq!(
        value["detail"],
        "did:plc:abc already holds a role on account 0192."
    );
    assert_eq!(value["status"], 409);
    // No stray `error` key from the old shape.
    assert!(value.get("error").is_none(), "the old shape is gone");
}

// AC4 — the response sets the problem+json content type (not application/json)
// and the HTTP status matching the body's `status`.
#[test]
fn into_response_sets_problem_json_content_type_and_status() {
    let response = Problem::forbidden().into_response();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .expect("content-type is set"),
        "application/problem+json"
    );
}

// ZMVP-66 AC3 — the fact-bearing delete refusal is a 409 whose detail points
// the caller at Archive (the path that remains once facts exist).
#[test]
fn commission_has_facts_is_a_409_pointing_at_archive() {
    let problem = Problem::commission_has_facts();
    assert_eq!(problem.r#type, "urn:zurfur:error:commission-has-facts");
    assert_eq!(problem.code, "commission_has_facts");
    assert_eq!(problem.status, 409);
    assert!(
        problem.detail.to_lowercase().contains("archive"),
        "the detail points at Archive, got {:?}",
        problem.detail
    );
}

// The 422 specifics share the invalid-request type but carry their own code.
#[test]
fn invalid_request_specifics_share_the_type_but_vary_the_code() {
    assert_eq!(Problem::invalid_request("x").code, "invalid_request");
    assert_eq!(Problem::unknown_role("bad").code, "unknown_role");
    assert_eq!(
        Problem::unknown_role("bad").r#type,
        "urn:zurfur:error:invalid-request"
    );
    assert_eq!(Problem::unknown_role("bad").status, 422);
}
