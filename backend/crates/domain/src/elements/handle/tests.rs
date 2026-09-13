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

// ---- Reserved labels (ZMVP-45) ----------------------------------------

#[test]
fn rejects_reserved_labels_in_zurfur_namespace() {
    for label in ["api", "admin", "www"] {
        assert_eq!(
            format!("{label}.zurfur.app").parse::<Handle>(),
            Err(HandleError::ReservedLabel(label.into())),
            "{label}.zurfur.app should be reserved"
        );
    }
}

// The platform root handle is reserved too — the leading-dot suffix check
// alone would let the bare apex slip past.
#[test]
fn rejects_the_bare_platform_apex() {
    assert_eq!(
        "zurfur.app".parse::<Handle>(),
        Err(HandleError::ReservedLabel("zurfur".into()))
    );
}

#[test]
fn accepts_a_normal_zurfur_subdomain() {
    assert_eq!(
        "alice.zurfur.app".parse::<Handle>().unwrap().as_str(),
        "alice.zurfur.app"
    );
}

#[test]
fn accepts_reserved_word_on_byo_domain() {
    // The reserved set guards only the *.zurfur.app namespace.
    assert_eq!(
        "api.example.com".parse::<Handle>().unwrap().as_str(),
        "api.example.com"
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

// ---- Error quality -----------------------------------------------------

#[test]
fn every_error_variant_renders_a_message() {
    let variants = [
        HandleError::Empty,
        HandleError::TooLong(300),
        HandleError::TooFewSegments,
        HandleError::EmptySegment,
        HandleError::SegmentTooLong(64),
        HandleError::InvalidChar('_'),
        HandleError::HyphenEdge,
        HandleError::TldLeadingDigit,
        HandleError::ReservedTld("local".into()),
        HandleError::PunycodeLabel,
        HandleError::ReservedLabel("api".into()),
    ];
    for v in variants {
        assert!(!v.to_string().is_empty(), "{v:?} rendered an empty message");
    }
}

// ---- The configured namespace (HandleDomain) -------------------------

fn domain(raw: &str) -> HandleDomain {
    raw.parse().expect("a valid handle domain")
}

#[test]
fn domain_normalizer_strips_case_whitespace_and_dots() {
    assert_eq!(domain(" Zurfur.App. ").as_str(), "zurfur.app");
    assert_eq!(domain(".zurfur.app").as_str(), "zurfur.app");
    assert_eq!(domain("zurfur.app").as_str(), "zurfur.app");
}

#[test]
fn domain_rejects_an_empty_value() {
    // An empty namespace would make every handle look like a member.
    assert_eq!("".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    assert_eq!("   ".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
    assert_eq!("...".parse::<HandleDomain>(), Err(HandleDomainError::Empty));
}

#[test]
fn domain_error_renders_a_message() {
    assert!(!HandleDomainError::Empty.to_string().is_empty());
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
    // The apex is not a member of its own namespace. `zurfur.app` is not a
    // constructible Handle, so another domain stands in for the shape.
    let apex_domain = domain("example.com");
    let apex = "example.com".parse::<Handle>().expect("a valid handle");
    assert!(!apex.is_in_namespace(&apex_domain));
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
