use serde_json::json;

use super::*;

const PLC: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";

fn did(text: &str) -> Did {
    Did::from(text.to_string())
}

/// The URL `text`'s document is read from, as text.
fn document_url(text: &str) -> Result<String, UnresolvableDid> {
    let DocumentUrl(url) = DocumentUrl::try_from(&did(text))?;
    Ok(url.as_ref().to_string())
}

/// A full document for `id` claiming `also_known_as`, with `services`.
fn body(id: &str, also_known_as: &[&str], services: serde_json::Value) -> Vec<u8> {
    let document = json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": id,
        "alsoKnownAs": also_known_as,
        "verificationMethod": [],
        "service": services,
    });
    serde_json::to_vec(&document).expect("serializes")
}

fn pds_service(endpoint: &str) -> serde_json::Value {
    json!([{
        "id": "#atproto_pds",
        "type": "AtprotoPersonalDataServer",
        "serviceEndpoint": endpoint,
    }])
}

fn parse(also_known_as: &[&str]) -> ResolvedDocument {
    let body = body(PLC, also_known_as, pds_service("https://pds.example.com"));
    ResolvedDocument::parse(&did(PLC), &body).expect("a full document parses")
}

fn handle(text: &str) -> AtHandle {
    text.parse().expect("a valid handle")
}

// --- the two URL shapes ---

#[test]
fn a_did_plc_is_read_from_the_plc_directory() {
    let expected = Ok(format!("https://plc.directory/{PLC}"));
    assert_eq!(document_url(PLC), expected);
}

#[test]
fn a_malformed_did_plc_is_refused_before_any_url() {
    for text in [
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaa",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaaa",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaa1",
        "did:plc:AAAAAAAAAAAAAAAAAAAAAAAA",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaa%",
    ] {
        assert_eq!(document_url(text), Err(UnresolvableDid), "{text}");
    }
}

#[test]
fn a_hostname_level_did_web_is_read_from_its_well_known_file() {
    let expected = Ok("https://alice.example.com/.well-known/did.json".to_string());
    assert_eq!(document_url("did:web:alice.example.com"), expected);
}

#[test]
fn a_did_web_with_a_port_a_path_or_a_non_public_host_is_refused() {
    for text in [
        "did:web:alice.example.com%3A8443",
        "did:web:alice.example.com:user:alice",
        "did:web:alice.example.com:443",
        "did:web:localhost",
        "did:web:localhost%3A3000",
        "did:web:127.0.0.1",
        "did:web:2130706433",
        "did:web:printer.local",
        "did:web:alice.example.com.",
        "did:web:",
    ] {
        assert_eq!(document_url(text), Err(UnresolvableDid), "{text}");
    }
}

#[test]
fn other_methods_are_refused() {
    for text in [
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        "did:example:123",
        "did:plcx:aaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert_eq!(document_url(text), Err(UnresolvableDid), "{text}");
    }
}

#[test]
fn a_did_over_2048_characters_is_refused() {
    let host = format!("{}.example.com", ["a"; 1100].join("."));
    let text = format!("did:web:{host}");
    assert!(text.len() > 2048);
    assert_eq!(document_url(&text), Err(UnresolvableDid));
}

// --- the strict parse ---

#[test]
fn a_document_naming_another_did_is_not_found() {
    let body = body(
        "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
        &[],
        pds_service("https://pds.example.com"),
    );
    let result = ResolvedDocument::parse(&did(PLC), &body);
    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[test]
fn a_slingshot_mini_doc_is_not_a_document() {
    let mini_doc = json!({
        "did": PLC,
        "handle": "alice.example.com",
        "pds": "https://pds.example.com",
        "signing_key": "zQ3shdiQ4srzeSN8V49TzxWCbNsvj2ZBWaFAB2AS2ii3vTiYK",
    });
    let body = serde_json::to_vec(&mini_doc).expect("serializes");
    let result = ResolvedDocument::parse(&did(PLC), &body);
    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}

#[test]
fn a_body_that_is_not_json_is_not_found() {
    for body in [&b"<html>"[..], b"", b"null", b"[]"] {
        let result = ResolvedDocument::parse(&did(PLC), body);
        assert!(matches!(result, Err(ResolveError::NotFound)), "{body:?}");
    }
}

// --- the claimed handle ---

#[test]
fn the_first_valid_at_uri_is_the_claim() {
    let document = parse(&[
        "https://alice.example.com",
        "at://not a handle",
        "at://Alice.Example.com",
        "at://bob.example.com",
    ]);
    let expected = Claim::Handle(handle("alice.example.com"));
    assert_eq!(document.claim(), expected);
}

#[test]
fn a_first_valid_entry_under_a_disallowed_tld_is_an_unusable_claim() {
    let document = parse(&["at://alice.local", "at://alice.example.com"]);
    assert_eq!(document.claim(), Claim::Unusable);
}

#[test]
fn padded_or_trailing_dot_entries_are_not_valid_and_are_skipped() {
    let document = parse(&[
        "at:// alice.example.com",
        "at://alice.example.com.",
        "at://bob.example.com",
    ]);
    let expected = Claim::Handle(handle("bob.example.com"));
    assert_eq!(document.claim(), expected);
}

#[test]
fn a_document_without_handles_claims_nothing() {
    assert_eq!(parse(&[]).claim(), Claim::Absent);
    assert_eq!(parse(&["https://alice.example.com"]).claim(), Claim::Absent);
}

// --- the PDS endpoint ---

#[test]
fn the_pds_is_the_atproto_pds_service() {
    let services = json!([
        {"id": "#atproto_pds", "type": "SomethingElse", "serviceEndpoint": "https://wrong.example.com"},
        {"id": "#other", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://other.example.com"},
        {"id": format!("{PLC}#atproto_pds"), "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://pds.example.com"},
    ]);
    let body = body(PLC, &[], services);
    let document = ResolvedDocument::parse(&did(PLC), &body).expect("parses");

    let pds = document.pds().expect("a PDS");
    assert_eq!(pds.as_ref(), "https://pds.example.com/");
}

#[test]
fn a_pds_the_url_policy_refuses_is_refused() {
    for endpoint in [
        "http://pds.example.com",
        "https://pds.example.com:8443",
        "https://127.0.0.1",
        "https://localhost",
    ] {
        let body = body(PLC, &[], pds_service(endpoint));
        let document = ResolvedDocument::parse(&did(PLC), &body).expect("parses");
        let result = document.pds();
        assert!(
            matches!(result, Err(ResolveError::Refused(_))),
            "{endpoint}: {result:?}"
        );
    }
}

#[test]
fn a_document_without_a_pds_has_none() {
    let body = body(PLC, &[], json!([]));
    let document = ResolvedDocument::parse(&did(PLC), &body).expect("parses");
    let result = document.pds();
    assert!(matches!(result, Err(ResolveError::NotFound)), "{result:?}");
}
