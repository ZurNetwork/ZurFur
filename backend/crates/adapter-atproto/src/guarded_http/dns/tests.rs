use std::net::IpAddr;

use super::*;
use crate::guarded_http::{
    limits::DNS_TIMEOUT,
    scripted::{Script, ScriptedLookup},
};

/// A filter over `lookup` with the production timeout.
fn filter_over(lookup: Arc<ScriptedLookup>) -> FilteringDns {
    FilteringDns::new(lookup, DNS_TIMEOUT)
}

fn addresses(texts: &[&str]) -> Vec<IpAddr> {
    texts
        .iter()
        .map(|text| text.parse().expect("test address parses"))
        .collect()
}

#[tokio::test]
async fn a_name_that_resolves_to_a_private_address_is_refused() {
    let lookup = ScriptedLookup::default()
        .with("pds.example.com.", Script::Answer(addresses(&["10.0.0.1"])));
    let filter = filter_over(Arc::new(lookup));

    let result = filter.public_addresses("pds.example.com").await;

    assert!(
        matches!(result, Err(DnsError::Refused(_))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn one_forbidden_answer_refuses_the_whole_name() {
    let lookup = ScriptedLookup::default().with(
        "pds.example.com.",
        Script::Answer(addresses(&["93.184.216.34", "127.0.0.1"])),
    );
    let filter = filter_over(Arc::new(lookup));

    let result = filter.public_addresses("pds.example.com").await;

    assert!(
        matches!(result, Err(DnsError::Refused(_))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn public_answers_resolve() {
    let answers = addresses(&["93.184.216.34", "2606:4700:4700::1111"]);
    let lookup =
        ScriptedLookup::default().with("pds.example.com.", Script::Answer(answers.clone()));
    let filter = filter_over(Arc::new(lookup));

    let resolved = filter
        .public_addresses("pds.example.com")
        .await
        .expect("public answers resolve");

    let resolved: Vec<IpAddr> = resolved.into_iter().map(IpAddr::from).collect();
    assert_eq!(resolved, answers);
}

#[tokio::test]
async fn an_empty_answer_is_not_found() {
    let lookup = ScriptedLookup::default().with("pds.example.com.", Script::Answer(Vec::new()));
    let filter = filter_over(Arc::new(lookup));

    let result = filter.public_addresses("pds.example.com").await;

    assert!(matches!(result, Err(DnsError::NotFound)), "got: {result:?}");
}

#[tokio::test]
async fn a_name_with_no_records_is_not_found() {
    let filter = filter_over(Arc::new(ScriptedLookup::default()));

    let result = filter.public_addresses("nowhere.example.com").await;

    assert!(matches!(result, Err(DnsError::NotFound)), "got: {result:?}");
}

#[tokio::test]
async fn a_lookup_error_is_unavailable() {
    let lookup = ScriptedLookup::default().with("pds.example.com.", Script::Fail);
    let filter = filter_over(Arc::new(lookup));

    let result = filter.public_addresses("pds.example.com").await;

    assert!(
        matches!(result, Err(DnsError::Unavailable(_))),
        "got: {result:?}"
    );
}

#[tokio::test]
async fn a_lookup_slower_than_the_timeout_is_unavailable() {
    let lookup = ScriptedLookup::default().with("pds.example.com.", Script::Hang);
    let short_timeout = Duration::from_millis(50);
    let filter = FilteringDns::new(Arc::new(lookup), short_timeout);

    // Bounded from outside, so a missing inner timeout fails instead of hanging.
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        filter.public_addresses("pds.example.com"),
    )
    .await
    .expect("the filter's own timeout fires first");

    assert!(
        matches!(result, Err(DnsError::Unavailable(_))),
        "got: {result:?}"
    );
}

#[test]
fn the_production_dns_timeout_is_five_seconds() {
    let five_seconds = Duration::from_secs(5);

    assert_eq!(DNS_TIMEOUT, five_seconds);
}

#[tokio::test]
async fn the_lookup_receives_the_fully_qualified_name() {
    let lookup = Arc::new(ScriptedLookup::default());
    let filter = filter_over(lookup.clone());

    let _ = filter.public_addresses("pds.example.com").await;
    let _ = filter.public_addresses("already.example.com.").await;

    let expected = vec![
        "pds.example.com.".to_string(),
        "already.example.com.".to_string(),
    ];
    assert_eq!(lookup.asked(), expected);
}

#[tokio::test]
async fn a_dns_error_never_echoes_the_name() {
    let lookup = ScriptedLookup::default().with(
        "secret.example.com.",
        Script::Answer(addresses(&["10.0.0.1"])),
    );
    let filter = filter_over(Arc::new(lookup));

    let error = filter
        .public_addresses("secret.example.com")
        .await
        .expect_err("refused");

    assert!(!error.to_string().contains("secret"));
}
