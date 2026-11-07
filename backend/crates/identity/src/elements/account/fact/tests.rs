use std::str::FromStr;

use strum::VariantArray;

use super::*;

#[test]
fn the_member_fact_is_stored_as_member() {
    let token: &'static str = AccountFactKind::Member.into();

    assert_eq!(token, "member");
    assert_eq!(AccountFactKind::Member.to_string(), "member");
}

#[test]
fn every_kind_round_trips_through_its_token() {
    for kind in AccountFactKind::VARIANTS {
        let token: &'static str = kind.into();
        assert_eq!(AccountFactKind::from_str(token), Ok(*kind));
    }
}

#[test]
fn an_unknown_token_is_refused_with_its_text() {
    let refused = AccountFactKind::from_str("Member");

    assert_eq!(refused, Err(UnknownAccountFactKind("Member".to_owned())));
}

#[test]
fn facts_hold_each_kind_once() {
    let facts = AccountFacts::from(vec![AccountFactKind::Member, AccountFactKind::Member]);

    assert!(!facts.is_empty());
    assert!(facts.contains(AccountFactKind::Member));
    assert_eq!(facts.iter().collect::<Vec<_>>(), [AccountFactKind::Member]);
}

#[test]
fn no_facts_is_empty() {
    let facts: AccountFacts = std::iter::empty().collect();

    assert!(facts.is_empty());
    assert_eq!(facts, AccountFacts::default());
}
