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

// The Den's one not-found: every miss answers these exact bytes, so the
// problem itself can never tell an absent node from a hidden one.
#[test]
fn node_not_found_is_one_fixed_404() {
    let expected = serde_json::json!({
        "type": "urn:zurfur:error:node-not-found",
        "code": "node_not_found",
        "title": "Node not found",
        "detail": "No such node.",
        "status": 404,
    });

    let first = serde_json::to_value(Problem::node_not_found()).expect("serializes");
    let second = serde_json::to_value(Problem::node_not_found()).expect("serializes");

    assert_eq!(first, expected);
    assert_eq!(second, expected);
}

// An unknown query parameter is a fixed 400 under the general bad-request
// type, and its detail never names the parameter it refused.
#[test]
fn unknown_parameter_is_one_fixed_400() {
    let expected = serde_json::json!({
        "type": "urn:zurfur:error:bad-request",
        "code": "unknown_parameter",
        "title": "Bad request",
        "detail": "The request carries a query parameter this endpoint doesn't accept.",
        "status": 400,
    });

    let problem = serde_json::to_value(Problem::unknown_parameter()).expect("serializes");

    assert_eq!(problem, expected);
}

// The 400 renders with its own status line, like every registry entry.
#[test]
fn unknown_parameter_responds_400() {
    let response = Problem::unknown_parameter().into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// A known query parameter with a value the endpoint can't use is one fixed
// 422: the general invalid-request type and code, with a detail that names
// neither the parameter nor its value.
#[test]
fn invalid_query_is_one_fixed_422() {
    let expected = serde_json::json!({
        "type": "urn:zurfur:error:invalid-request",
        "code": "invalid_request",
        "title": "Invalid request",
        "detail": "The request's query string carries a value this endpoint can't use.",
        "status": 422,
    });

    let problem = serde_json::to_value(Problem::invalid_query()).expect("serializes");

    assert_eq!(problem, expected);
}
