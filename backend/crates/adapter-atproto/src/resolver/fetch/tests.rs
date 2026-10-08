use super::*;
use crate::guarded_http::FetchError;

/// A response with `status` and a small body.
fn response(status: u16) -> http::Response<Vec<u8>> {
    let mut response = http::Response::new(b"body".to_vec());
    *response.status_mut() = http::StatusCode::from_u16(status).expect("a valid status");
    response
}

#[test]
fn a_2xx_answer_hands_back_its_body() {
    let expected_body = b"body".to_vec();
    for status in [200, 203, 204] {
        let body = successful_body(response(status)).expect("a 2xx is a body");
        assert_eq!(body, expected_body, "{status}");
    }
}

#[test]
fn a_missing_gone_or_otherwise_refused_answer_is_not_found() {
    for status in [400, 401, 403, 404, 410, 451] {
        let result = successful_body(response(status));
        assert!(
            matches!(result, Err(ResolveError::NotFound)),
            "{status}: {result:?}"
        );
    }
}

#[test]
fn a_server_error_a_timeout_or_a_rate_limit_is_unavailable() {
    for status in [408, 429, 500, 502, 503, 504] {
        let result = successful_body(response(status));
        assert!(
            matches!(result, Err(ResolveError::Unavailable(_))),
            "{status}: {result:?}"
        );
    }
}

#[test]
fn a_fetch_failure_keeps_its_class() {
    let refused = FetchError::Refused(crate::guarded_http::Refusal::Redirect);
    let unavailable = FetchError::Unavailable("timed out".into());

    let not_found = ResolveError::from(FetchError::NotFound);
    let refused = ResolveError::from(refused);
    let unavailable = ResolveError::from(unavailable);

    assert!(matches!(not_found, ResolveError::NotFound), "{not_found:?}");
    assert!(matches!(refused, ResolveError::Refused(_)), "{refused:?}");
    assert!(
        matches!(unavailable, ResolveError::Unavailable(_)),
        "{unavailable:?}"
    );
}
