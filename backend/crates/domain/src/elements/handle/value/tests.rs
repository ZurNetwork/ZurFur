use super::*;

// ---- Normalization -----------------------------------------------------

#[test]
fn lowercases_the_handle() {
    assert_eq!(
        "Alice.Zurfur.APP".parse::<Handle>().unwrap().as_str(),
        "alice.zurfur.app"
    );
}

#[test]
fn strips_a_single_trailing_dot() {
    assert_eq!(
        "alice.zurfur.app.".parse::<Handle>().unwrap().as_str(),
        "alice.zurfur.app"
    );
}

#[test]
fn trims_surrounding_whitespace() {
    assert_eq!(
        "  alice.example.com  ".parse::<Handle>().unwrap().as_str(),
        "alice.example.com"
    );
}

// ---- Charset / segment / length ---------------------------------------

#[test]
fn rejects_a_single_segment() {
    assert_eq!("alice".parse::<Handle>(), Err(HandleError::TooFewSegments));
}

#[test]
fn rejects_an_empty_input() {
    assert_eq!("   ".parse::<Handle>(), Err(HandleError::Empty));
    assert_eq!(".".parse::<Handle>(), Err(HandleError::Empty));
}

#[test]
fn rejects_an_empty_segment() {
    assert_eq!(
        "alice..app".parse::<Handle>(),
        Err(HandleError::EmptySegment)
    );
}

#[test]
fn rejects_a_segment_over_63_chars() {
    let long_label = "a".repeat(64);
    assert_eq!(
        format!("{long_label}.app").parse::<Handle>(),
        Err(HandleError::SegmentTooLong(64))
    );
}

#[test]
fn rejects_a_handle_over_253_chars() {
    // Build a >253-char handle out of legal 63-char labels.
    let label = "a".repeat(63);
    let raw = format!("{label}.{label}.{label}.{label}.com"); // 4*63 + 3 + 4 = 259
    let len = raw.chars().count();
    assert_eq!(raw.parse::<Handle>(), Err(HandleError::TooLong(len)));
}

#[test]
fn rejects_a_leading_or_trailing_hyphen() {
    assert_eq!("-alice.app".parse::<Handle>(), Err(HandleError::HyphenEdge));
    assert_eq!("alice-.app".parse::<Handle>(), Err(HandleError::HyphenEdge));
}

#[test]
fn rejects_out_of_charset_bytes() {
    assert_eq!(
        "ali_ce.app".parse::<Handle>(),
        Err(HandleError::InvalidChar('_'))
    );
    assert_eq!(
        "ali ce.app".parse::<Handle>(),
        Err(HandleError::InvalidChar(' '))
    );
    assert_eq!(
        "café.app".parse::<Handle>(),
        Err(HandleError::InvalidChar('é'))
    );
}

#[test]
fn lowercases_ascii_only() {
    // U+212A KELVIN SIGN lowercases to an ASCII `k` under Unicode rules; the
    // spec lowercases ASCII only, so it stays a foreign character and fails.
    let kelvin_sign = '\u{212A}';
    let raw = format!("{kelvin_sign}itty.zurfur.app");
    assert_eq!(
        raw.parse::<Handle>(),
        Err(HandleError::InvalidChar(kelvin_sign))
    );
}

#[test]
fn rejects_a_digit_leading_tld() {
    assert_eq!(
        "alice.123".parse::<Handle>(),
        Err(HandleError::TldLeadingDigit)
    );
}

// ---- Reserved TLDs -----------------------------------------------------

#[test]
fn rejects_reserved_tlds() {
    assert_eq!(
        "foo.local".parse::<Handle>(),
        Err(HandleError::ReservedTld("local".into()))
    );
    assert_eq!(
        "foo.test".parse::<Handle>(),
        Err(HandleError::ReservedTld("test".into()))
    );
    assert_eq!(
        "foo.onion".parse::<Handle>(),
        Err(HandleError::ReservedTld("onion".into()))
    );
}

// ---- Punycode (ZMVP-48) -----------------------------------------------

#[test]
fn rejects_punycode_zurfur_label() {
    assert_eq!(
        "xn--80ak6aa92e.zurfur.app".parse::<Handle>(),
        Err(HandleError::PunycodeLabel)
    );
}

#[test]
fn rejects_punycode_byo_domain() {
    assert_eq!(
        "xn--e1awd7f.com".parse::<Handle>(),
        Err(HandleError::PunycodeLabel)
    );
}

#[test]
fn rejects_punycode_anywhere_and_mixed_case() {
    // Not just the leftmost label.
    assert_eq!(
        "good.xn--abc.com".parse::<Handle>(),
        Err(HandleError::PunycodeLabel)
    );
    // Mixed-case `XN--` is normalized then caught.
    assert_eq!(
        "XN--abc.com".parse::<Handle>(),
        Err(HandleError::PunycodeLabel)
    );
}

// ---- Reserved labels -----------------------------------------------------

fn handle(raw: &str) -> Handle {
    raw.parse().expect("a valid handle")
}

// Parsing is context-free: a reserved label only fails at the claim, against
// the configured namespace.
#[test]
fn parsing_accepts_a_reserved_label() {
    assert_eq!(handle("admin.zurfur.app").as_str(), "admin.zurfur.app");
    assert_eq!(handle("zurfur.app").as_str(), "zurfur.app");
}

#[test]
fn refuses_reserved_labels_in_the_configured_namespace() {
    for namespace in ["zurfur.app", "zurfur.foo"] {
        let configured = domain(namespace);
        for label in RESERVED_LABELS {
            let reserved = handle(&format!("{label}.{namespace}"));
            let reserved_label_error = HandleError::ReservedLabel((*label).into());
            assert_eq!(
                reserved.ensure_unreserved(&configured),
                Err(reserved_label_error),
                "{reserved} should be reserved under {namespace}"
            );
        }
    }
}

// Zurfur Studio is served from `studio.zurfur.app`, so no one may claim it.
#[test]
fn rejects_the_studio_product_host() {
    let studio = handle("studio.zurfur.app");
    let reserved_label_error = HandleError::ReservedLabel("studio".into());
    assert_eq!(
        studio.ensure_unreserved(&domain("zurfur.app")),
        Err(reserved_label_error)
    );
}

// Staging's namespace gates its own handles, not production's: under
// `zurfur.foo`, an `admin.zurfur.app` is a BYO handle like any other.
#[test]
fn the_gate_follows_the_configured_namespace() {
    let staging = domain("zurfur.foo");
    let production_admin = handle("admin.zurfur.app");
    assert_eq!(production_admin.ensure_unreserved(&staging), Ok(()));
    let staging_admin = handle("admin.zurfur.foo");
    let reserved_label_error = HandleError::ReservedLabel("admin".into());
    assert_eq!(
        staging_admin.ensure_unreserved(&staging),
        Err(reserved_label_error)
    );
}

// The leftmost label is the one gated, however deep the member.
#[test]
fn refuses_a_reserved_leftmost_label_on_a_deeper_member() {
    let deep = handle("api.alice.zurfur.app");
    let reserved_label_error = HandleError::ReservedLabel("api".into());
    assert_eq!(
        deep.ensure_unreserved(&domain("zurfur.app")),
        Err(reserved_label_error)
    );
    let nested_ordinary = handle("alice.api.zurfur.app");
    assert_eq!(
        nested_ordinary.ensure_unreserved(&domain("zurfur.app")),
        Ok(())
    );
}

// The namespace's root handle is withheld whatever its label: no one claims
// the platform apex.
#[test]
fn refuses_the_bare_apex_of_the_configured_namespace() {
    assert_eq!(
        handle("zurfur.app").ensure_unreserved(&domain("zurfur.app")),
        Err(HandleError::NamespaceApex)
    );
    assert_eq!(
        handle("stage-example.net").ensure_unreserved(&domain("stage-example.net")),
        Err(HandleError::NamespaceApex)
    );
}

// The apex has its own message: it is not "a reserved label".
#[test]
fn the_apex_refusal_names_the_apex_not_a_label() {
    let apex = handle("zurfur.app");
    let Err(refusal) = apex.ensure_unreserved(&domain("zurfur.app")) else {
        panic!("the apex must be refused");
    };
    let message = refusal.to_string();
    assert!(message.contains("apex"), "{message}");
    assert!(!message.contains("reserved label"), "{message}");
}

#[test]
fn accepts_an_ordinary_member_and_a_reserved_word_on_a_byo_domain() {
    let configured = domain("zurfur.app");
    assert_eq!(
        handle("alice.zurfur.app").ensure_unreserved(&configured),
        Ok(())
    );
    // The reserved set guards only the configured namespace.
    assert_eq!(
        handle("api.example.com").ensure_unreserved(&configured),
        Ok(())
    );
    // A look-alike is not a member, so it is not gated either.
    assert_eq!(
        handle("api.notzurfur.app").ensure_unreserved(&configured),
        Ok(())
    );
}

#[test]
fn accepts_a_normal_zurfur_subdomain() {
    assert_eq!(
        "alice.zurfur.app".parse::<Handle>().unwrap().as_str(),
        "alice.zurfur.app"
    );
}

// ---- Happy path --------------------------------------------------------

#[test]
fn accepts_well_formed_handles() {
    assert_eq!(
        "alice.zurfur.app".parse::<Handle>().unwrap().as_str(),
        "alice.zurfur.app"
    );
    assert_eq!(
        "alice.example.com".parse::<Handle>().unwrap().as_str(),
        "alice.example.com"
    );
}

// ---- The configured namespace (HandleDomain) -------------------------

fn domain(raw: &str) -> HandleDomain {
    raw.parse().expect("a valid handle domain")
}

#[test]
fn namespace_membership_is_a_strict_subdomain() {
    let alice = "alice.zurfur.app"
        .parse::<Handle>()
        .expect("a valid handle");
    assert!(alice.is_in_namespace(&domain("zurfur.app")));
    // The same namespace however deployment spelled it.
    assert!(alice.is_in_namespace(&domain("Zurfur.App.")));
    assert!(alice.is_in_namespace(&domain(".zurfur.app")));
    // A brought (BYO) domain is not a member.
    let byo = "alice.example.com"
        .parse::<Handle>()
        .expect("a valid handle");
    assert!(!byo.is_in_namespace(&domain("zurfur.app")));
}

#[test]
fn namespace_membership_refuses_the_apex_and_lookalikes() {
    let zurfur = domain("zurfur.app");
    // The apex is not a member of its own namespace.
    let apex = "zurfur.app".parse::<Handle>().expect("a valid handle");
    assert!(!apex.is_in_namespace(&zurfur));
    // A look-alike that ends with the domain's *text* but not on a label
    // boundary is refused — the leading dot is the boundary.
    for look_alike in ["notzurfur.app", "evil-zurfur.app", "xzurfur.app"] {
        let handle = look_alike.parse::<Handle>().expect("a valid handle");
        assert!(
            !handle.is_in_namespace(&zurfur),
            "{look_alike} must not be in the zurfur.app namespace"
        );
    }
    // A handle that merely *contains* the domain mid-string is refused too.
    let embedded = "zurfur.app.evil.com"
        .parse::<Handle>()
        .expect("a valid handle");
    assert!(!embedded.is_in_namespace(&zurfur));
}
