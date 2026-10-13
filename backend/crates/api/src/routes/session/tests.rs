use super::*;
use axum::http::HeaderValue;
use strum::VariantArray;

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
fn each_sign_in_failure_keeps_its_wire_code() {
    let codes: Vec<&str> = SignInFailure::VARIANTS.iter().map(AsRef::as_ref).collect();
    let expected = [
        "denied",
        "invalid_callback",
        "exchange_failed",
        "account_mismatch",
        "browser_mismatch",
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
fn an_exchange_failure_is_generic_unless_the_account_or_browser_mismatched() {
    let account_mismatch = anyhow::Error::new(AccountMismatch);
    let browser_mismatch = anyhow::Error::new(BrowserMismatch);
    let other = anyhow::anyhow!("the PDS refused the code");
    assert_eq!(
        callback_failure(&account_mismatch),
        SignInFailure::AccountMismatch
    );
    assert_eq!(
        callback_failure(&browser_mismatch),
        SignInFailure::BrowserMismatch
    );
    assert_eq!(callback_failure(&other), SignInFailure::ExchangeFailed);
}

#[test]
fn a_browser_refusal_wrapping_its_class_still_maps_to_browser_mismatch() {
    let class = anyhow::anyhow!("no browser binding");
    let refusal = class.context(BrowserMismatch);
    assert_eq!(callback_failure(&refusal), SignInFailure::BrowserMismatch);
}

#[test]
fn only_a_failure_that_matched_the_binding_clears_it() {
    let clears: Vec<(&str, bool)> = SignInFailure::VARIANTS
        .iter()
        .map(|failure| (failure.as_ref(), failure.matched_the_binding()))
        .collect();
    let expected = [
        ("denied", false),
        ("invalid_callback", false),
        ("exchange_failed", false),
        ("account_mismatch", true),
        ("browser_mismatch", false),
    ];
    assert_eq!(clears, expected);
}
