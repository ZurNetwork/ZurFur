//! Shared helpers for the api integration tests.

/// Assert that `res` is an RFC 9457 `application/problem+json` error carrying the
/// given HTTP `status` and our terse `code` — and that none of the old
/// `{ "error": string }` shape remains (ZMVP-35). The exact `type` URN per code is
/// pinned by the unit tests in `problem.rs`; here we assert the contract shape.
pub async fn assert_problem(res: reqwest::Response, status: u16, code: &str) {
    assert_eq!(res.status().as_u16(), status, "unexpected HTTP status");
    assert_eq!(
        res.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .expect("content-type is set"),
        "application/problem+json",
        "errors are RFC 9457 problem+json",
    );
    let body: serde_json::Value = res.json().await.expect("problem body is JSON");
    assert_eq!(body["code"], code, "unexpected problem code");
    assert_eq!(
        body["status"], status,
        "body status mirrors the HTTP status"
    );
    assert!(
        body["type"]
            .as_str()
            .is_some_and(|t| t.starts_with("urn:zurfur:error:")),
        "type is a urn:zurfur:error URN, got {:?}",
        body["type"],
    );
    assert!(
        body.get("error").is_none(),
        "the old {{ error }} shape is gone",
    );
}
