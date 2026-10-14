use super::*;
use axum::{Router, routing::get};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, session::Id};

use crate::session_layer;

/// The session cookie's name.
const SID: &str = "zurfur.sid";

/// A `HeaderMap` holding one `Cookie` header per entry of `cookies`, in order.
fn cookie_headers(cookies: &[&str]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for cookie in cookies {
        let value = HeaderValue::from_str(cookie).expect("a header value");
        headers.append(COOKIE, value);
    }
    headers
}

/// The `Cookie` headers in `headers`, as text, in order.
fn cookie_texts(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .map(|value| value.to_str().expect("visible ASCII").to_owned())
        .collect()
}

/// The `Cookie` headers left once `cookies` pass the guard.
fn guarded(cookies: &[&str]) -> Vec<String> {
    let mut headers = cookie_headers(cookies);
    drop_contested_pairs(&mut headers, SID);
    cookie_texts(&headers)
}

#[test]
fn one_pair_passes_untouched() {
    let cookies = ["theme=dark; zurfur.sid=valid; zurfur.signin=tok"];

    let left = guarded(&cookies);

    assert_eq!(left, cookies);
}

#[test]
fn one_pair_split_from_other_cookies_passes_untouched() {
    let cookies = ["theme=dark", "zurfur.sid=valid", "other=1"];

    let left = guarded(&cookies);

    assert_eq!(left, cookies);
}

#[test]
fn two_pairs_in_one_header_are_both_dropped_in_either_order() {
    let cases = [
        "theme=dark; zurfur.sid=valid; zurfur.sid=planted; other=1",
        "theme=dark; zurfur.sid=planted; zurfur.sid=valid; other=1",
    ];

    for cookie in cases {
        let left = guarded(&[cookie]);

        let others_only = ["theme=dark; other=1"];
        assert_eq!(left, others_only, "for {cookie:?}");
    }
}

#[test]
fn two_identical_pairs_are_both_dropped() {
    let left = guarded(&["zurfur.sid=valid; zurfur.sid=valid; other=1"]);

    let others_only = [" other=1"];
    assert_eq!(left, others_only);
}

#[test]
fn pairs_split_across_headers_are_counted_together_and_dropped() {
    let cases = [
        ["zurfur.sid=valid; theme=dark", "zurfur.sid=planted"],
        ["zurfur.sid=planted", "theme=dark; zurfur.sid=valid"],
    ];

    for cookies in cases {
        let left = guarded(&cookies);

        // A header left with no pair is removed; the others keep theirs.
        assert_eq!(left.len(), 1, "for {cookies:?}");
        assert_eq!(left[0].trim(), "theme=dark", "for {cookies:?}");
    }
}

#[test]
fn the_sign_in_binding_cookie_is_kept_when_the_session_pairs_go() {
    let left = guarded(&["zurfur.sid=valid; zurfur.signin=tok; zurfur.sid=planted"]);

    let binding_only = [" zurfur.signin=tok"];
    assert_eq!(left, binding_only);
}

#[test]
fn a_percent_encoded_name_counts_as_the_session_cookie_as_the_jar_decodes_it() {
    let left = guarded(&["zurfur.sid=valid; zurfur%2Esid=planted; other=1"]);

    let others_only = [" other=1"];
    assert_eq!(left, others_only);
}

#[test]
fn lookalike_names_are_not_session_pairs() {
    let cookies = ["zurfur.sidx=1; zurfur.sid=valid; xzurfur.sid=2"];

    let left = guarded(&cookies);

    assert_eq!(left, cookies);
}

#[test]
fn a_header_that_is_not_utf8_is_kept_as_is() {
    let mut headers = cookie_headers(&["zurfur.sid=valid", "zurfur.sid=planted"]);
    let opaque = HeaderValue::from_bytes(b"blob=\xff\xfe").expect("an opaque header value");
    headers.append(COOKIE, opaque.clone());

    drop_contested_pairs(&mut headers, SID);

    let left: Vec<&HeaderValue> = headers.get_all(COOKIE).iter().collect();
    assert_eq!(left, [&opaque]);
}

/// The session id each request carries when it reaches a handler behind
/// `router`'s session middleware, sent with the `Cookie` headers in `cookies`;
/// an empty string for none.
async fn seen_session_ids(router: Router, cookies: &[&[&str]]) -> Vec<String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serves");
    });

    let client = reqwest::Client::new();
    let mut seen = Vec::new();
    for headers in cookies {
        let mut request = client.get(format!("http://{addr}/"));
        for header in *headers {
            request = request.header(reqwest::header::COOKIE, *header);
        }
        let response = request.send().await.expect("a response");
        seen.push(response.text().await.expect("a body"));
    }
    seen
}

/// A router answering with the session id its middleware read from the request.
fn session_id_echo() -> Router {
    Router::new().route(
        "/",
        get(async |session: Session| session.id().map(|id| id.to_string()).unwrap_or_default()),
    )
}

/// Two well-formed session ids, the one a visitor holds and one planted beside it.
fn two_ids() -> (String, String) {
    (Id(1).to_string(), Id(2).to_string())
}

#[tokio::test]
async fn bare_tower_sessions_reads_the_last_of_two_pairs() {
    let (valid, planted) = two_ids();
    let bare = SessionManagerLayer::new(MemoryStore::default()).with_name(SID);
    let router = session_id_echo().layer(bare);
    let one_header = format!("{SID}={planted}; {SID}={valid}");
    let first = format!("{SID}={planted}");
    let second = format!("{SID}={valid}");

    let seen = seen_session_ids(router, &[&[&one_header], &[&first, &second]]).await;

    // tower-cookies keeps the last pair of a name, across headers too.
    assert_eq!(seen, [valid.clone(), valid]);
}

#[tokio::test]
async fn the_session_layer_reads_no_session_from_two_pairs() {
    let (valid, planted) = two_ids();
    let router = session_id_echo().layer(session_layer(MemoryStore::default(), false));
    let alone = format!("{SID}={valid}");
    let valid_first = format!("{SID}={valid}; {SID}={planted}");
    let planted_first = format!("{SID}={planted}; {SID}={valid}");
    let identical = format!("{SID}={valid}; {SID}={valid}");
    let planted_pair = format!("{SID}={planted}");

    let requests: [&[&str]; 6] = [
        &[&alone],
        &[&valid_first],
        &[&planted_first],
        &[&identical],
        &[&alone, &planted_pair],
        &[&planted_pair, &alone],
    ];

    let seen = seen_session_ids(router, &requests).await;

    let only_the_lone_pair_is_read = [
        valid,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    assert_eq!(seen, only_the_lone_pair_is_read);
}
