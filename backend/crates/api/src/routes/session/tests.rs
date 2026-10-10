use super::*;

#[test]
fn where_cookies_are_secure_the_binding_is_host_locked() {
    let lifetime = std::time::Duration::from_secs(600);
    let cookie = browser_binding_cookie(true, "token", lifetime).expect("a header value");
    let expected =
        "__Host-zurfur.signin=token; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=600";
    assert_eq!(cookie, expected);
}

#[test]
fn clearing_the_binding_keeps_its_name_path_and_flags() {
    let cleared_secure = cleared_browser_binding_cookie(true);
    let expected_secure =
        "__Host-zurfur.signin=; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=0";
    assert_eq!(cleared_secure, expected_secure);
    let cleared_dev = cleared_browser_binding_cookie(false);
    let expected_dev = "zurfur.signin=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0";
    assert_eq!(cleared_dev, expected_dev);
}

#[test]
fn where_cookies_are_secure_only_the_host_locked_name_is_read() {
    let mut headers = HeaderMap::new();
    let planted = HeaderValue::from_static("zurfur.signin=planted; __Host-zurfur.signin=real");
    headers.insert(header::COOKIE, planted);
    let presented = presented_browser_binding(&headers, true).map(|b| b.as_ref().to_owned());
    assert_eq!(presented.as_deref(), Some("real"));

    let mut only_planted = HeaderMap::new();
    let unprefixed = HeaderValue::from_static("zurfur.signin=planted");
    only_planted.insert(header::COOKIE, unprefixed);
    assert!(presented_browser_binding(&only_planted, true).is_none());
}

#[test]
fn a_token_that_cannot_be_a_header_is_refused() {
    let lifetime = std::time::Duration::from_secs(600);
    assert!(browser_binding_cookie(false, "bad\nvalue", lifetime).is_none());
}
