use super::*;
use strum::VariantArray;

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

#[test]
fn each_sign_in_failure_keeps_its_wire_code() {
    let codes: Vec<&str> = SignInFailure::VARIANTS.iter().map(AsRef::as_ref).collect();
    let expected = [
        "denied",
        "invalid_callback",
        "exchange_failed",
        "account_mismatch",
    ];
    assert_eq!(codes, expected);
}

#[test]
fn a_failure_redirects_to_login_on_the_public_origin() {
    let public_url = Url::parse("https://zurfur.example").expect("a URL");
    let location = login_location(&public_url, SignInFailure::AccountMismatch);
    assert_eq!(
        location.as_str(),
        "https://zurfur.example/login?error=account_mismatch"
    );
}

#[test]
fn home_is_the_root_of_the_public_origin() {
    let public_url = Url::parse("https://zurfur.example").expect("a URL");
    assert_eq!(
        home_location(&public_url).as_str(),
        "https://zurfur.example/"
    );
}

#[test]
fn an_exchange_failure_is_generic_unless_the_account_mismatched() {
    let mismatch = anyhow::Error::new(AccountMismatch);
    let other = anyhow::anyhow!("the PDS refused the code");
    assert_eq!(callback_failure(&mismatch), SignInFailure::AccountMismatch);
    assert_eq!(callback_failure(&other), SignInFailure::ExchangeFailed);
}
