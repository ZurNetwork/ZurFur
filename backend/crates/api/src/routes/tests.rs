use super::*;

#[test]
fn the_public_origin_is_first_party_whatever_its_path() {
    let public_url = Url::parse("https://zurfur.example/app/").expect("a URL");
    assert!(is_first_party(b"https://zurfur.example", &public_url));
}

#[test]
fn another_scheme_host_or_port_is_not_first_party() {
    let public_url = Url::parse("http://127.0.0.1:8080").expect("a URL");
    for origin in [
        "https://127.0.0.1:8080",
        "http://localhost:8080",
        "http://127.0.0.1:8081",
        "http://127.0.0.1:8080/",
    ] {
        assert!(!is_first_party(origin.as_bytes(), &public_url), "{origin}");
    }
}

#[test]
fn an_opaque_public_origin_matches_not_even_null() {
    let public_url = Url::parse("data:text/plain,zurfur").expect("a URL");
    assert!(!is_first_party(b"null", &public_url));
}
