use super::*;
use crate::elements::handle::HandleError;

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

// A namespace is a bare hostname: a wildcard, a port, a scheme or a path would
// make every membership test lie.
#[test]
fn domain_rejects_anything_but_a_bare_hostname() {
    let not_hostnames = [
        ("*.zurfur.app", HandleError::InvalidChar('*')),
        ("zurfur.app:443", HandleError::InvalidChar(':')),
        ("https://zurfur.app", HandleError::InvalidChar(':')),
        ("zurfur.app/", HandleError::InvalidChar('/')),
        ("127.0.0.1", HandleError::TldLeadingDigit),
        ("zurfur", HandleError::TooFewSegments),
        // Only ASCII is lowercased, as for a handle: the Kelvin sign never
        // folds into `k`.
        ("\u{212A}urfur.app", HandleError::InvalidChar('\u{212A}')),
    ];
    for (raw, rule) in not_hostnames {
        let refusal = HandleDomainError::NotAHostname(rule);
        assert_eq!(raw.parse::<HandleDomain>(), Err(refusal), "{raw}");
    }
}

// A namespace whose members could never parse as handles is refused too.
#[test]
fn domain_rejects_a_namespace_no_handle_could_live_under() {
    let punycode = HandleDomainError::NotAHostname(HandleError::PunycodeLabel);
    assert_eq!("xn--80ak6aa92e.app".parse::<HandleDomain>(), Err(punycode));
    let reserved_tld = HandleDomainError::NotAHostname(HandleError::ReservedTld("local".into()));
    assert_eq!("zurfur.local".parse::<HandleDomain>(), Err(reserved_tld));
}
