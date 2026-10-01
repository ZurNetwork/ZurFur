use super::*;
use crate::plc::PlcOperation;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn signed_op() -> crate::plc::SignedOperation {
    PlcOperation::identity_only(
        vec!["did:key:a".to_string(), "did:key:b".to_string()],
        "did:key:c".to_string(),
        "alice.zurfur.app",
    )
    .into_signed("sig".to_string())
}

// The HTTP submitter must (1) trim a trailing slash on the base URL so the
// target path is single-slashed `/{did}`, and (2) surface a non-2xx response as
// an error carrying the status and body. `expect(1)` + `body_json` pin both the
// exact path and the exact JSON body the submitter sent.
#[tokio::test]
async fn a_rejection_is_an_error_and_the_path_is_single_slashed() {
    let server = MockServer::start().await;
    let op = signed_op().to_json().unwrap();
    Mock::given(method("POST"))
        .and(path("/did:plc:x"))
        .and(body_json(&op))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad"))
        .expect(1)
        .mount(&server)
        .await;

    // Base URL carries a trailing slash on purpose — it must be trimmed.
    let dir = HttpPlcDirectory::new(format!("{}/", server.uri()));
    let err = dir.submit("did:plc:x", &op).await.unwrap_err();

    let message = err.to_string();
    assert!(message.contains("400"), "got: {message}");
    assert!(message.contains("bad"), "got: {message}");
    server.verify().await;
}

// The counterpart to the rejection case: a 2xx response is Ok, on the same
// single-slashed path and JSON body.
#[tokio::test]
async fn an_accepted_submission_is_ok() {
    let server = MockServer::start().await;
    let op = signed_op().to_json().unwrap();
    Mock::given(method("POST"))
        .and(path("/did:plc:x"))
        .and(body_json(&op))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let dir = HttpPlcDirectory::new(format!("{}/", server.uri()));
    let res = dir.submit("did:plc:x", &op).await;

    assert!(res.is_ok(), "got: {res:?}");
    server.verify().await;
}
