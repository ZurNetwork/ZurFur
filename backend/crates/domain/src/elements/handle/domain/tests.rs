use super::*;

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
