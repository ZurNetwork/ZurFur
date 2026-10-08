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

// --- the system DNS's conversions of hickory's answers ---

mod system {
    use hickory_resolver::net::{DnsError as HickoryDnsError, NoRecords};
    use hickory_resolver::proto::op::{Query, ResponseCode};
    use hickory_resolver::proto::rr::{Name, rdata::TXT};

    use super::*;

    fn txt_record(pieces: &[&str]) -> Record {
        let pieces = pieces.iter().map(|piece| (*piece).to_string()).collect();
        Record::from_rdata(Name::root(), 300, RData::TXT(TXT::new(pieces)))
    }

    #[test]
    fn a_txt_record_split_into_pieces_is_joined_in_order() {
        let records = [
            txt_record(&["did=did:web:", "alice.example.com"]),
            txt_record(&["v=spf1 -all"]),
        ];

        let values = txt_values(&records);

        let expected = vec![
            b"did=did:web:alice.example.com".to_vec(),
            b"v=spf1 -all".to_vec(),
        ];
        assert_eq!(values, expected);
    }

    #[test]
    fn records_that_are_not_txt_are_skipped() {
        let address = Record::from_rdata(
            Name::root(),
            300,
            RData::A("93.184.216.34".parse().expect("an address")),
        );
        let records = [
            address,
            txt_record(&["did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa"]),
        ];

        let values = txt_values(&records);

        let expected = vec![b"did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa".to_vec()];
        assert_eq!(values, expected);
    }

    #[test]
    fn nxdomain_and_an_empty_answer_are_no_records() {
        for code in [ResponseCode::NXDomain, ResponseCode::NoError] {
            let no_records = NoRecords::new(Query::default(), code);
            let error = NetError::from(no_records);
            let converted = lookup_error(error);
            assert!(
                matches!(converted, LookupError::NoRecords),
                "{code:?}: {converted:?}"
            );
        }
    }

    #[test]
    fn servfail_and_other_failures_are_failures_never_no_records() {
        let failures = [
            NetError::from(HickoryDnsError::ResponseCode(ResponseCode::ServFail)),
            NetError::from(HickoryDnsError::ResponseCode(ResponseCode::Refused)),
            NetError::from(HickoryDnsError::ResponseCode(ResponseCode::FormErr)),
            NetError::Timeout,
        ];
        for error in failures {
            let shown = error.to_string();
            let converted = lookup_error(error);
            assert!(
                matches!(converted, LookupError::Failed(_)),
                "{shown}: {converted:?}"
            );
        }
    }
}
