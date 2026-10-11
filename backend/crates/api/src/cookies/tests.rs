use super::*;
use axum::{Router, routing::get};
use strum::VariantArray;
use tower_sessions::{MemoryStore, Session};

/// `cookie` exactly as its `Set-Cookie` header carries it.
fn header_text(cookie: &Cookie<'_>) -> String {
    let header = set_cookie_header(cookie).expect("a header value");
    header.to_str().expect("visible ASCII").to_owned()
}

#[test]
fn each_cookie_keeps_its_wire_names() {
    let names: Vec<(&str, &str)> = ZurfurCookie::VARIANTS
        .iter()
        .map(|cookie| (cookie.name(true), cookie.name(false)))
        .collect();
    let expected = [
        ("zurfur.sid", "zurfur.sid"),
        ("__Host-zurfur.signin", "zurfur.signin"),
    ];
    assert_eq!(names, expected);
}

#[test]
fn where_cookies_are_secure_the_binding_is_host_locked() {
    let max_age = time::Duration::seconds(600);
    let binding = ZurfurCookie::SignInBinding.cookie(true, "token", max_age);
    let expected =
        "__Host-zurfur.signin=token; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=600";
    assert_eq!(header_text(&binding), expected);
}

#[test]
fn where_cookies_are_not_secure_the_binding_has_no_prefix() {
    let max_age = time::Duration::seconds(600);
    let binding = ZurfurCookie::SignInBinding.cookie(false, "token", max_age);
    let expected = "zurfur.signin=token; HttpOnly; SameSite=Lax; Path=/; Max-Age=600";
    assert_eq!(header_text(&binding), expected);
}

#[test]
fn clearing_the_binding_keeps_its_name_path_and_flags() {
    let cleared_secure = ZurfurCookie::SignInBinding.cleared(true);
    let expected_secure =
        "__Host-zurfur.signin=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0";
    assert_eq!(header_text(&cleared_secure), expected_secure);

    let cleared_dev = ZurfurCookie::SignInBinding.cleared(false);
    let expected_dev = "zurfur.signin=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";
    assert_eq!(header_text(&cleared_dev), expected_dev);
}

#[test]
fn the_session_cookie_is_set_and_cleared_with_the_same_flags() {
    let max_age = time::Duration::days(7);
    let session_secure = ZurfurCookie::Session.cookie(true, "id", max_age);
    let expected_secure = "zurfur.sid=id; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=604800";
    assert_eq!(header_text(&session_secure), expected_secure);

    let session_dev = ZurfurCookie::Session.cookie(false, "id", max_age);
    let expected_dev = "zurfur.sid=id; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800";
    assert_eq!(header_text(&session_dev), expected_dev);

    let cleared_secure = ZurfurCookie::Session.cleared(true);
    let expected_cleared_secure = "zurfur.sid=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0";
    assert_eq!(header_text(&cleared_secure), expected_cleared_secure);

    let cleared_dev = ZurfurCookie::Session.cleared(false);
    let expected_cleared_dev = "zurfur.sid=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";
    assert_eq!(header_text(&cleared_dev), expected_cleared_dev);
}

#[test]
fn a_value_that_cannot_be_a_header_is_refused() {
    let max_age = time::Duration::seconds(600);
    let binding = ZurfurCookie::SignInBinding.cookie(false, "bad\nvalue", max_age);
    assert!(set_cookie_header(&binding).is_none());
}

/// The `Set-Cookie` the session layer emits when a request writes the session.
async fn session_layer_set_cookie(secure: bool) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let router = Router::new()
        .route(
            "/",
            get(async |session: Session| {
                session.insert("seen", true).await.expect("a session write");
            }),
        )
        .layer(session_layer(MemoryStore::default(), secure));
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serves");
    });

    let response = reqwest::get(format!("http://{addr}/"))
        .await
        .expect("a response");
    let set_cookie = response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .expect("a Set-Cookie");
    set_cookie.to_str().expect("visible ASCII").to_owned()
}

/// The random session id in a `zurfur.sid` `Set-Cookie`.
fn session_id(set_cookie: &str) -> &str {
    set_cookie
        .split(';')
        .next()
        .and_then(|pair| pair.strip_prefix("zurfur.sid="))
        .expect("a zurfur.sid pair")
}

#[tokio::test]
async fn the_session_layer_issues_the_session_cookie_where_cookies_are_secure() {
    let set_cookie = session_layer_set_cookie(true).await;
    let id = session_id(&set_cookie);
    let expected =
        format!("zurfur.sid={id}; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=604800");
    assert_eq!(set_cookie, expected);
}

#[tokio::test]
async fn the_session_layer_issues_the_session_cookie_where_cookies_are_not_secure() {
    let set_cookie = session_layer_set_cookie(false).await;
    let id = session_id(&set_cookie);
    let expected = format!("zurfur.sid={id}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800");
    assert_eq!(set_cookie, expected);
}
