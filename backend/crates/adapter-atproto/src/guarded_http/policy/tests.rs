use super::*;

/// Every listed URL that the policy does NOT refuse with `expected`, paired
/// with what it got instead.
fn mismatches(
    urls: &[&str],
    expected: UrlPolicyError,
) -> Vec<(String, Result<(), UrlPolicyError>)> {
    urls.iter()
        .map(|text| {
            let verdict = PublicHttpsUrl::try_from(*text).map(|_| ());
            (text.to_string(), verdict)
        })
        .filter(|(_, verdict)| *verdict != Err(expected))
        .collect()
}

#[test]
fn a_public_https_url_is_admitted() {
    let text = "https://bsky.social/";

    let admitted = PublicHttpsUrl::try_from(text).expect("admitted");

    let expected = url::Url::parse(text).expect("parses");
    assert_eq!(url::Url::from(admitted), expected);
}

#[test]
fn an_explicit_port_443_and_a_trailing_dot_on_a_public_name_are_admitted() {
    let urls = ["https://bsky.social:443/xrpc", "https://bsky.social./"];

    let refused: Vec<&str> = urls
        .into_iter()
        .filter(|text| PublicHttpsUrl::try_from(*text).is_err())
        .collect();

    let none: Vec<&str> = Vec::new();
    assert_eq!(refused, none);
}

#[test]
fn every_other_scheme_is_refused() {
    let urls = [
        "http://bsky.social/",
        "file:///etc/passwd",
        "data:text/plain,hello",
        "ftp://bsky.social/",
        "wss://bsky.social/",
    ];

    let wrong = mismatches(&urls, UrlPolicyError::NotHttps);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn a_port_other_than_443_is_refused() {
    let urls = [
        "https://bsky.social:8443/",
        "https://bsky.social:80/",
        "https://bsky.social:0/",
    ];

    let wrong = mismatches(&urls, UrlPolicyError::Port);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn userinfo_is_refused() {
    let urls = [
        "https://user@bsky.social/",
        "https://user:pass@bsky.social/",
        "https://:pass@bsky.social/",
    ];

    let wrong = mismatches(&urls, UrlPolicyError::Userinfo);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn an_ip_literal_is_refused_in_every_spelling() {
    let urls = [
        "https://127.0.0.1/",
        "https://127.0.0.1./",
        "https://2130706433/",
        "https://0x7f.1/",
        "https://0177.0.0.1/",
        "https://127.1/",
        "https://[::1]/",
        "https://[::ffff:7f00:1]/",
        // Public addresses too: no host Zurfur fetches needs a literal.
        "https://8.8.8.8/",
        "https://[2606:4700:4700::1111]/",
    ];

    let wrong = mismatches(&urls, UrlPolicyError::IpLiteral);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn local_reserved_and_single_label_names_are_refused() {
    let urls = [
        "https://localhost/",
        "https://localhost./",
        "https://x.localhost/",
        "https://printer.local/",
        "https://x.local./",
        "https://a.internal/",
        "https://x.onion/",
        "https://a.alt/",
        "https://x.arpa/",
        "https://site.example/",
        "https://x.invalid/",
        "https://pds.test/",
        "https://intranet/",
        "https://a..com/",
        "https://bsky.social../",
    ];

    let wrong = mismatches(&urls, UrlPolicyError::LocalName);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn text_that_is_not_a_url_is_refused() {
    let urls = ["not a url", "", "https://"];

    let wrong = mismatches(&urls, UrlPolicyError::NotUrl);

    assert_eq!(wrong, Vec::new());
}

#[test]
fn the_refusal_never_echoes_the_url() {
    let secret_host = "secret-host.local";
    let text = format!("https://{secret_host}/");

    let error = PublicHttpsUrl::try_from(text.as_str()).expect_err("refused");

    assert!(!error.to_string().contains(secret_host));
}
