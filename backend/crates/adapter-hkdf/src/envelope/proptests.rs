use proptest::prelude::*;

use super::*;
use crate::derivation::{key_id, purpose_key};
use crate::golden::{subject, test_root};

/// Read `bytes` as an envelope and, when it parses, open it under the DID custody key:
/// the full open path past the key-id check.
fn read_and_open(bytes: &[u8]) -> Result<OpenedSecret, CustodyError> {
    let root = test_root();
    let key = purpose_key(&root, Purpose::DidCustody);
    let parts = EnvelopeParts::try_from(bytes)?;
    open(
        &parts,
        &key,
        SealPurpose::DidCustody,
        &subject("account_keys"),
    )
}

/// A v1 header naming the test root's DID custody key id.
fn held_header() -> Vec<u8> {
    let root = test_root();
    let held_key_id = key_id(&root, Purpose::DidCustody);
    let key_id_bytes = held_key_id.as_ref().as_bytes();
    let key_id_length = u8::try_from(key_id_bytes.len()).expect("a short key id");
    [&[ENVELOPE_VERSION, key_id_length], key_id_bytes].concat()
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_panic_and_never_open(
        bytes in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        prop_assert!(read_and_open(&bytes).is_err());
    }

    #[test]
    fn arbitrary_bodies_under_a_held_key_id_never_open(
        body in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let bytes = [held_header().as_slice(), body.as_slice()].concat();

        let opened = read_and_open(&bytes).err();

        let expected = if body.len() < NONCE_BYTES + AEAD_TAG_BYTES {
            CustodyError::Malformed
        } else {
            CustodyError::Unauthentic
        };
        prop_assert_eq!(opened, Some(expected));
    }
}
