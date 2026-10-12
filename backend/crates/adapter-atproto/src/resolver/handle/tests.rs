use super::*;

/// The TXT answer for these record values.
fn answer(values: &[&[u8]]) -> TxtAnswer {
    values.iter().copied().collect()
}

fn did(text: &str) -> Did {
    Did::from(text.to_string())
}

/// A valid handle of exactly `len` characters, in labels of at most 63.
fn handle_of_len(len: usize) -> AtHandle {
    let mut text = String::from("com");
    while text.len() < len {
        let room = len - text.len() - 1;
        let label = "a".repeat(room.clamp(1, 63));
        text = format!("{label}.{text}");
    }
    assert_eq!(text.len(), len, "the helper builds an exact length");
    text.parse().expect("a valid long handle")
}

const ALICE: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn one_did_record_names_its_did() {
    let expected = TxtAnswer::Names(did(ALICE));
    assert_eq!(answer(&[b"did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa"]), expected);
}

#[test]
fn values_not_starting_did_equals_are_ignored() {
    let records: &[&[u8]] = &[
        b"v=spf1 -all",
        b"DID=did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
        b"did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
    ];
    let expected = TxtAnswer::Names(did(ALICE));
    assert_eq!(answer(records), expected);
}

#[test]
fn records_naming_different_dids_conflict() {
    let records: &[&[u8]] = &[
        b"did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        b"did=did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
    ];
    assert_eq!(answer(records), TxtAnswer::Conflicting);
}

#[test]
fn the_same_did_twice_is_one_answer() {
    let records: &[&[u8]] = &[
        b"did=did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
        b"did=did:plc:bbbbbbbbbbbbbbbbbbbbbbbb",
    ];
    let expected = TxtAnswer::Names(did(BOB));
    assert_eq!(answer(records), expected);
}

#[test]
fn no_record_holding_a_did_leaves_https_to_decide() {
    let records: &[&[u8]] = &[b"did=not a did", b"did=", b"did=\xff\xfe", b"hello"];
    assert_eq!(answer(records), TxtAnswer::Silent);
    assert_eq!(answer(&[]), TxtAnswer::Silent);
}

#[test]
fn the_well_known_file_is_its_first_line_trimmed() {
    let expected = Some(did(ALICE));
    for body in [
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa\n",
        "  did:plc:aaaaaaaaaaaaaaaaaaaaaaaa \r\n",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa\nanything after",
    ] {
        assert_eq!(well_known_did(body.as_bytes()), expected, "{body:?}");
    }
}

#[test]
fn a_well_known_file_that_is_not_a_did_names_nothing() {
    for body in [
        &b""[..],
        b"\ndid:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        b"<html>hello</html>",
        b"did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        b"\xff\xfe",
    ] {
        assert_eq!(well_known_did(body), None, "{body:?}");
    }
}

#[test]
fn the_txt_name_is_fully_qualified() {
    let handle: AtHandle = "alice.example.com".parse().expect("valid");
    let expected = Some("_atproto.alice.example.com.".to_string());
    assert_eq!(txt_name(&handle), expected);
}

#[test]
fn a_handle_too_long_for_dns_has_no_txt_name() {
    let longest = handle_of_len(244);
    let too_long = handle_of_len(245);

    let fitting = txt_name(&longest).expect("244 characters fit");
    assert_eq!(txt_name(&too_long), None);

    // `_atproto.` + 244 + the root: 255 octets on the wire, DNS's limit.
    let wire_octets = fitting.trim_end_matches('.').len() + 2;
    assert_eq!(wire_octets, 255);
}
