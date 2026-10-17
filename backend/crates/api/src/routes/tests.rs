use axum::http::HeaderName;

use super::*;

const PUBLIC_URL: &str = "http://127.0.0.1:8080";

fn public_url() -> Url {
    Url::parse(PUBLIC_URL).expect("a URL")
}

/// A header map holding every `(name, value)` pair, repeats appended.
fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        let name = HeaderName::try_from(*name).expect("a header name");
        let value = HeaderValue::try_from(*value).expect("a header value");
        headers.append(name, value);
    }
    headers
}

#[test]
fn the_public_origin_is_first_party_whatever_its_path() {
    let public_url = Url::parse("https://zurfur.example/app/").expect("a URL");
    assert!(names_origin(b"https://zurfur.example", &public_url));
}

#[test]
fn another_scheme_host_or_port_is_not_first_party() {
    for origin in [
        "https://127.0.0.1:8080",
        "http://localhost:8080",
        "http://127.0.0.1:8081",
        "http://127.0.0.1:8080/",
        "HTTP://127.0.0.1:8080",
    ] {
        assert!(!names_origin(origin.as_bytes(), &public_url()), "{origin}");
    }
}

#[test]
fn an_opaque_public_origin_matches_not_even_null() {
    let public_url = Url::parse("data:text/plain,zurfur").expect("a URL");
    assert!(!names_origin(b"null", &public_url));
}

#[test]
fn a_matching_origin_passes_whatever_the_fetch_metadata_says() {
    for site in ["cross-site", "same-site", "none", "Same-Origin"] {
        let request = headers(&[("origin", PUBLIC_URL), ("sec-fetch-site", site)]);
        assert!(is_first_party(&request, &public_url()), "{site}");
    }
}

#[test]
fn a_matching_origin_passes_beside_two_fetch_sites() {
    let request = headers(&[
        ("origin", PUBLIC_URL),
        ("sec-fetch-site", "cross-site"),
        ("sec-fetch-site", "same-origin"),
    ]);
    assert!(is_first_party(&request, &public_url()));
}

#[test]
fn two_origins_or_two_fetch_sites_are_refused() {
    let two_origins = headers(&[("origin", PUBLIC_URL), ("origin", PUBLIC_URL)]);
    let two_fetch_sites = headers(&[
        ("sec-fetch-site", "same-origin"),
        ("sec-fetch-site", "same-origin"),
    ]);
    let two_origins_and_same_origin = headers(&[
        ("origin", PUBLIC_URL),
        ("origin", PUBLIC_URL),
        ("sec-fetch-site", "same-origin"),
    ]);
    assert!(!is_first_party(&two_origins, &public_url()));
    assert!(!is_first_party(&two_fetch_sites, &public_url()));
    assert!(!is_first_party(&two_origins_and_same_origin, &public_url()));
}

#[test]
fn fetch_site_is_matched_exactly() {
    let exact = headers(&[("sec-fetch-site", "same-origin")]);
    assert!(is_first_party(&exact, &public_url()));
    for site in [
        "Same-Origin",
        "SAME-ORIGIN",
        "same-origin ",
        "same-origin, same-origin",
        "",
    ] {
        let request = headers(&[("sec-fetch-site", site)]);
        assert!(!is_first_party(&request, &public_url()), "{site:?}");
    }
}

#[test]
fn no_signal_at_all_is_not_first_party() {
    assert!(!is_first_party(&HeaderMap::new(), &public_url()));
}

#[test]
fn only_get_head_and_options_are_safe() {
    for safe in [Method::GET, Method::HEAD, Method::OPTIONS] {
        assert!(is_safe(&safe), "{safe}");
    }
    let writes = [
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::TRACE,
        Method::CONNECT,
        Method::from_bytes(b"get").expect("a method"),
        Method::from_bytes(b"patch").expect("a method"),
        Method::from_bytes(b"PROPFIND").expect("a method"),
    ];
    for write in writes {
        assert!(!is_safe(&write), "{write}");
    }
}
