use serde_json::Value;

use super::*;
use crate::golden::{array_at, bytes_at, golden, hex_text, subject, text_at};

/// One golden envelope's inputs, read from the golden file.
struct GoldenEnvelope {
    purpose: SealPurpose,
    key: [u8; KEY_BYTES],
    key_id: KeyId,
    subject: CustodySubject,
    nonce: XNonce,
    plaintext: Vec<u8>,
    associated_data: Vec<u8>,
    header: Vec<u8>,
    ciphertext: Vec<u8>,
}

/// Every golden envelope.
fn golden_envelopes() -> Vec<GoldenEnvelope> {
    let golden = golden();
    array_at(&golden, "/envelopes")
        .iter()
        .map(|envelope| golden_envelope(&golden, envelope))
        .collect()
}

fn golden_envelope(golden: &Value, envelope: &Value) -> GoldenEnvelope {
    let token = text_at(envelope, "/purpose");
    let purpose: SealPurpose = token.parse().expect("a golden seal purpose");
    let key_bytes = bytes_at(golden, &format!("/keys/{token}/key"));
    let key = <[u8; KEY_BYTES]>::try_from(key_bytes.as_slice()).expect("a 32-byte golden key");
    let key_id = KeyId::try_from(text_at(golden, &format!("/keys/{token}/key_id")))
        .expect("a golden key id");
    let nonce = XNonce::clone_from_slice(&bytes_at(envelope, "/nonce"));
    GoldenEnvelope {
        purpose,
        key,
        key_id,
        subject: subject(text_at(envelope, "/subject")),
        nonce,
        plaintext: bytes_at(envelope, "/plaintext"),
        associated_data: bytes_at(envelope, "/associated_data"),
        header: bytes_at(envelope, "/header"),
        ciphertext: bytes_at(envelope, "/ciphertext"),
    }
}

/// `golden`'s envelope sealed again under its fixed nonce.
fn reseal(golden: &GoldenEnvelope) -> SealedEnvelope {
    seal_with_nonce(
        &golden.key,
        &golden.key_id,
        golden.purpose,
        &golden.subject,
        &golden.nonce,
        &golden.plaintext,
    )
    .expect("a golden envelope seals")
}

#[test]
fn associated_data_matches_the_golden_file() {
    for golden in golden_envelopes() {
        let built = associated_data(golden.purpose, &golden.key_id, &golden.subject)
            .expect("golden fields fit");

        assert_eq!(hex_text(&built), hex_text(&golden.associated_data));
    }
}

#[test]
fn envelope_headers_match_the_golden_file() {
    for golden in golden_envelopes() {
        let sealed = reseal(&golden);
        let header = sealed
            .as_ref()
            .get(..golden.header.len())
            .unwrap_or_default();

        assert_eq!(hex_text(header), hex_text(&golden.header));
    }
}

#[test]
fn the_fixed_nonce_seal_reproduces_each_golden_envelope() {
    for golden in golden_envelopes() {
        let expected_envelope = [golden.header.as_slice(), golden.ciphertext.as_slice()].concat();

        let sealed = reseal(&golden);

        assert_eq!(hex_text(sealed.as_ref()), hex_text(&expected_envelope));
    }
}

#[test]
fn golden_envelopes_parse_into_their_parts() {
    for golden in golden_envelopes() {
        let envelope = [golden.header.as_slice(), golden.ciphertext.as_slice()].concat();

        let parts = EnvelopeParts::try_from(envelope.as_slice()).expect("a golden envelope parses");

        assert_eq!(parts.key_id(), &golden.key_id);
        assert_eq!(parts.nonce, golden.nonce);
        assert_eq!(parts.sealed, golden.ciphertext.as_slice());
    }
}

#[test]
fn distinct_fields_give_distinct_aad() {
    // Each of the first two pairs would collide if the field it shifts bytes across lost
    // its length prefix; the subject, last, can't be shifted past.
    let tuples: [(&[u8], &[u8], &[u8]); 6] = [
        (b"p\x00\x00\x00\x05", b"k", b"s"),
        (b"p", &[0, 0, 0, 1, b'k'], b"s"),
        (b"p", b"k", &[b's', 0, 0, 0, 0]),
        (b"p", &[b'k', 0, 0, 0, 5, b's'], &[]),
        (b"did_custody", b"hkdf:a", b"x"),
        (b"signin_secrets", b"hkdf:a", b"x"),
    ];

    let built: Vec<Vec<u8>> = tuples
        .iter()
        .map(|(purpose_token, key_id, subject)| {
            associated_data_of_fields(purpose_token, key_id, subject).expect("small fields fit")
        })
        .collect();

    for (index, associated_data) in built.iter().enumerate() {
        for (other_index, other) in built.iter().enumerate().skip(index + 1) {
            assert_ne!(
                associated_data, other,
                "fields {index} and {other_index} give the same associated data"
            );
        }
    }
}

#[test]
fn associated_data_reads_back_into_its_fields() {
    let fields: [&[u8]; 3] = [b"did_custody", b"hkdf:did_custody:00", &[0, 0, 0, 1, 0xff]];

    let built = associated_data_of_fields(fields[0], fields[1], fields[2]).expect("fields fit");

    let (version, mut rest) = built
        .split_first()
        .expect("the associated data is not empty");
    assert_eq!(*version, ENVELOPE_VERSION);
    for field in fields {
        let (length, tail) = rest.split_at(4);
        let length = u32::from_be_bytes(length.try_into().expect("a 4-byte length"));
        let (read, tail) = tail.split_at(usize::try_from(length).expect("a small length"));
        assert_eq!(read, field);
        rest = tail;
    }
    assert!(rest.is_empty());
}

#[test]
fn an_envelope_shorter_than_its_parts_is_malformed() {
    let key_id_text = "hkdf:did_custody:0011223344556677";
    let key_id_length = u8::try_from(key_id_text.len()).expect("a short key id");
    let header = [&[ENVELOPE_VERSION, key_id_length], key_id_text.as_bytes()].concat();
    let cases: [(&str, Vec<u8>); 5] = [
        ("empty", Vec::new()),
        ("the version alone", vec![ENVELOPE_VERSION]),
        (
            "a key id running past the end",
            header[..header.len() - 1].to_vec(),
        ),
        ("no nonce", header.clone()),
        (
            "a body shorter than nonce and tag",
            [header.as_slice(), &[0u8; NONCE_BYTES + AEAD_TAG_BYTES - 1]].concat(),
        ),
    ];

    for (case, bytes) in cases {
        let read = EnvelopeParts::try_from(bytes.as_slice()).err();

        assert_eq!(read, Some(CustodyError::Malformed), "{case}");
    }
}

#[test]
fn a_key_id_outside_the_alphabet_or_empty_is_malformed() {
    let cases: [(&str, Vec<u8>); 3] = [
        ("empty key id", vec![ENVELOPE_VERSION, 0]),
        ("upper case", [&[ENVELOPE_VERSION, 4][..], b"HKDF"].concat()),
        ("not utf-8", vec![ENVELOPE_VERSION, 2, 0xff, 0xfe]),
    ];

    for (case, header) in cases {
        let bytes = [header.as_slice(), &[0u8; NONCE_BYTES + AEAD_TAG_BYTES]].concat();

        let read = EnvelopeParts::try_from(bytes.as_slice()).err();

        assert_eq!(read, Some(CustodyError::Malformed), "{case}");
    }
}
