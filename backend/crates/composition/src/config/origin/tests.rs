use super::*;

fn origin(raw: &str) -> BareOrigin {
    raw.parse().expect("a bare origin")
}

// A scheme, a host and a port make an origin; a trailing `/` is no path.
#[test]
fn accepts_a_scheme_a_host_and_an_optional_port() {
    let bare = [
        "http://127.0.0.1",
        "http://127.0.0.1:8080",
        "http://127.0.0.1:8080/",
        "http://[::1]:8080",
        "https://zurfur.app",
        "https://zurfur.foo:8443/",
    ];
    for raw in bare {
        let parsed = raw.parse::<BareOrigin>();
        assert!(parsed.is_ok(), "{raw}: {parsed:?}");
    }
}

// Each rule refuses on its own, and says which one failed.
#[test]
fn refuses_each_part_past_the_origin() {
    let refused = [
        ("http://user@127.0.0.1:8080", BareOriginError::Credentials),
        (
            "http://user:secret@[::1]:8080",
            BareOriginError::Credentials,
        ),
        (
            "http://:secret@127.0.0.1:8080",
            BareOriginError::Credentials,
        ),
        ("https://:secret@zurfur.foo", BareOriginError::Credentials),
        ("https://zurfur.foo/app", BareOriginError::Path),
        ("https://zurfur.foo/app/", BareOriginError::Path),
        ("http://127.0.0.1:8080//", BareOriginError::Path),
        ("file:///srv/zurfur", BareOriginError::Path),
        ("data:text/plain,zurfur", BareOriginError::Path),
        ("http://127.0.0.1:8080?", BareOriginError::Query),
        ("https://zurfur.foo/?next=x", BareOriginError::Query),
        ("http://127.0.0.1:8080#", BareOriginError::Fragment),
        ("https://zurfur.foo/#top", BareOriginError::Fragment),
        ("file:///", BareOriginError::NoHost),
    ];
    for (raw, rule) in refused {
        assert_eq!(raw.parse::<BareOrigin>(), Err(rule), "{raw}");
    }
}

// A value that is no URL at all is refused with the parser's reason.
#[test]
fn refuses_a_value_that_is_not_a_url() {
    let not_a_url = BareOriginError::NotAUrl(url::ParseError::RelativeUrlWithoutBase);
    assert_eq!("zurfur.app".parse::<BareOrigin>(), Err(not_a_url));
    let bad_port = BareOriginError::NotAUrl(url::ParseError::InvalidPort);
    assert_eq!(
        "http://127.0.0.1:99999".parse::<BareOrigin>(),
        Err(bad_port)
    );
}

// No refusal echoes the value it refused.
#[test]
fn no_refusal_echoes_the_value() {
    let secret_bearing = [
        "http://user:hunter2@127.0.0.1:8080",
        "http://hunter2@127.0.0.1:99999",
        "https://zurfur.foo/hunter2",
        "https://zurfur.foo/?hunter2",
        "https://zurfur.foo/#hunter2",
    ];
    for raw in secret_bearing {
        let Err(refusal) = raw.parse::<BareOrigin>() else {
            panic!("{raw} must be refused");
        };
        let message = refusal.to_string();
        assert!(!message.contains("hunter2"), "{message}");
    }
}

// The URL stays reachable for the callers that build on it.
#[test]
fn the_url_is_reachable_through_as_ref() {
    let bare = origin("https://zurfur.app");
    let url: &Url = bare.as_ref();
    assert_eq!(url.as_str(), "https://zurfur.app/");
}

// Scheme and parsed host must both match, any port.
#[test]
fn is_of_origin_compares_scheme_and_parsed_host_any_port() {
    let loopback = ["http://127.0.0.1", "http://[::1]"];
    for raw in [
        "http://127.0.0.1",
        "http://127.0.0.1:8080",
        "http://[::1]:1",
    ] {
        assert!(origin(raw).is_of_origin(&loopback), "{raw}");
    }
    let outsiders = [
        "https://127.0.0.1:8080",
        "http://localhost:8080",
        "http://127.0.0.2:8080",
        "http://0.0.0.0:8080",
        "http://[::]:8080",
        "http://[::2]:8080",
        "http://[::ffff:127.0.0.1]:8080",
        "http://zurfur.foo",
    ];
    for raw in outsiders {
        assert!(!origin(raw).is_of_origin(&loopback), "{raw}");
    }
    // An unparsable entry in the list matches nothing.
    assert!(!origin("http://127.0.0.1").is_of_origin(&["not a url"]));
}

// https on a domain of two or more labels, not localhost, port not 0.
#[test]
fn is_public_https_wants_https_on_a_public_domain_name() {
    for raw in [
        "https://zurfur.foo",
        "https://zurfur.foo:8443",
        "https://a.b.zurfur.app",
    ] {
        assert!(origin(raw).is_public_https(), "{raw}");
    }
    let not_public = [
        "http://zurfur.foo",
        "http://127.0.0.1:8080",
        "https://203.0.113.7",
        "https://[2001:db8::1]",
        "https://zurfur.foo.",
        "https://a..b",
        "https://zurfur.foo:0",
        "https://localhost",
        "https://localhost:8443",
        "https://zurfur",
        "https://zurfur.localhost",
    ];
    for raw in not_public {
        assert!(!origin(raw).is_public_https(), "{raw}");
    }
}
