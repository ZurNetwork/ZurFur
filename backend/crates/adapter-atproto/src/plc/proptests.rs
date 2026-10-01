//! Property tests over the PLC op/tombstone builders. None of the
//! constructors validate their inputs, so arbitrary strings exercise the same
//! DAG-CBOR/hash paths the pinned vectors in `tests.rs` check at one point.

use super::*;
use proptest::collection::vec;
use proptest::prelude::*;

/// The five constructor inputs shared by every `plc_operation` builder.
#[derive(Clone, Debug)]
struct Inputs {
    rotation_keys: Vec<String>,
    signing_did: String,
    handle: String,
    prev: String,
    sig: String,
}

fn inputs() -> impl Strategy<Value = Inputs> {
    (
        vec(any::<String>(), 0..3),
        any::<String>(),
        any::<String>(),
        any::<String>(),
        any::<String>(),
    )
        .prop_map(|(rotation_keys, signing_did, handle, prev, sig)| Inputs {
            rotation_keys,
            signing_did,
            handle,
            prev,
            sig,
        })
}

fn genesis(input: &Inputs) -> PlcOperation {
    PlcOperation::identity_only(
        input.rotation_keys.clone(),
        input.signing_did.clone(),
        &input.handle,
    )
}

fn handleless(input: &Inputs) -> PlcOperation {
    PlcOperation::identity_only_handleless(input.rotation_keys.clone(), input.signing_did.clone())
}

fn update(input: &Inputs) -> PlcOperation {
    PlcOperation::update_handle(
        input.rotation_keys.clone(),
        input.signing_did.clone(),
        &input.handle,
        input.prev.clone(),
    )
}

/// Every base32 char PLC mints from a hash is RFC 4648 lowercase base32:
/// `[a-z2-7]`, never `0`/`1`/`8`/`9`.
fn is_plc_base32_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '2'..='7')
}

/// One `json_is_faithful` case: a builder plus its expected `alsoKnownAs` and
/// `prev`.
type JsonCase = (
    fn(&Inputs) -> PlcOperation,
    serde_json::Value,
    serde_json::Value,
);

proptest! {
    #[test]
    fn equal_inputs_give_equal_bytes_dids_and_cids(input in inputs()) {
        for build in [genesis as fn(&Inputs) -> PlcOperation, handleless, update] {
            let signing_bytes_1 = build(&input).signing_bytes().unwrap();
            let signing_bytes_2 = build(&input).signing_bytes().unwrap();
            prop_assert_eq!(signing_bytes_1, signing_bytes_2);

            let signed_1 = build(&input).into_signed(input.sig.clone());
            let signed_2 = build(&input).into_signed(input.sig.clone());
            prop_assert_eq!(signed_1.did().unwrap(), signed_2.did().unwrap());
            prop_assert_eq!(signed_1.cid().unwrap(), signed_2.cid().unwrap());
            prop_assert_eq!(signed_1.to_json().unwrap(), signed_2.to_json().unwrap());
        }
    }

    #[test]
    fn did_and_cid_have_their_shape(input in inputs()) {
        let signed = genesis(&input).into_signed(input.sig.clone());

        let did = signed.did().unwrap();
        let did_suffix = did.strip_prefix("did:plc:").expect("did:plc: prefix");
        prop_assert_eq!(did_suffix.chars().count(), 24);
        prop_assert!(did_suffix.chars().all(is_plc_base32_char));

        let cid = signed.cid().unwrap();
        let cid_suffix = cid.strip_prefix("bafyrei").expect("bafyrei prefix");
        prop_assert_eq!(cid_suffix.chars().count(), 52);
        prop_assert!(cid_suffix.chars().all(is_plc_base32_char));
    }

    #[test]
    fn json_is_faithful(input in inputs()) {
        let cases: [JsonCase; 3] = [
            (
                genesis,
                serde_json::json!([format!("at://{}", input.handle)]),
                serde_json::Value::Null,
            ),
            (handleless, serde_json::json!([]), serde_json::Value::Null),
            (
                update,
                serde_json::json!([format!("at://{}", input.handle)]),
                serde_json::json!(input.prev),
            ),
        ];

        for (build, expected_also_known_as, expected_prev) in cases {
            let json = build(&input).into_signed(input.sig.clone()).to_json().unwrap();

            prop_assert_eq!(&json["rotationKeys"], &serde_json::json!(input.rotation_keys));
            prop_assert_eq!(
                &json["verificationMethods"],
                &serde_json::json!({ "atproto": input.signing_did })
            );
            prop_assert_eq!(&json["alsoKnownAs"], &expected_also_known_as);
            prop_assert_eq!(&json["prev"], &expected_prev);
            prop_assert_eq!(&json["sig"], &serde_json::json!(input.sig));
            prop_assert_eq!(&json["services"], &serde_json::json!({}));
            prop_assert_eq!(
                json.as_object().unwrap().len(),
                7,
                "exactly 7 keys: type, rotationKeys, verificationMethods, alsoKnownAs, services, prev, sig"
            );
        }
    }

    #[test]
    fn the_signature_is_outside_the_signed_bytes(input in inputs(), sig2 in any::<String>()) {
        prop_assume!(input.sig != sig2);

        let signing_bytes_1 = genesis(&input).signing_bytes().unwrap();
        let signing_bytes_2 = genesis(&input).signing_bytes().unwrap();
        prop_assert_eq!(signing_bytes_1, signing_bytes_2);

        let cid_1 = genesis(&input).into_signed(input.sig.clone()).cid().unwrap();
        let cid_2 = genesis(&input).into_signed(sig2).cid().unwrap();
        prop_assert_ne!(cid_1, cid_2);
    }

    #[test]
    fn distinct_handles_give_distinct_hashes(input in inputs(), handle2 in any::<String>()) {
        prop_assume!(input.handle != handle2);
        let mut other = input.clone();
        other.handle = handle2;

        let genesis_1 = genesis(&input).into_signed(input.sig.clone());
        let genesis_2 = genesis(&other).into_signed(input.sig.clone());
        prop_assert_ne!(genesis_1.cid().unwrap(), genesis_2.cid().unwrap());
        prop_assert_ne!(genesis_1.did().unwrap(), genesis_2.did().unwrap());

        let update_1 = update(&input).into_signed(input.sig.clone());
        prop_assert_ne!(genesis_1.cid().unwrap(), update_1.cid().unwrap());
    }

    #[test]
    fn tombstone_is_deterministic_and_shaped(prev in any::<String>(), sig in any::<String>()) {
        let signed_1 = TombstoneOperation::new(prev.clone()).into_signed(sig.clone());
        let signed_2 = TombstoneOperation::new(prev.clone()).into_signed(sig.clone());

        let cid_1 = signed_1.cid().unwrap();
        let cid_2 = signed_2.cid().unwrap();
        prop_assert_eq!(&cid_1, &cid_2);

        let cid_suffix = cid_1.strip_prefix("bafyrei").expect("bafyrei prefix");
        prop_assert_eq!(cid_suffix.chars().count(), 52);
        prop_assert!(cid_suffix.chars().all(is_plc_base32_char));

        let json = signed_1.to_json().unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        prop_assert_eq!(keys, vec!["prev", "sig", "type"]);
    }
}
