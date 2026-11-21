use proptest::prelude::*;

use super::*;

/// A list of up to five parts, each up to eight bytes from a three-byte alphabet, so
/// lists that differ only in where they split are common.
fn part_list() -> impl Strategy<Value = Vec<Vec<u8>>> {
    let part = prop::collection::vec(prop::sample::select(vec![0u8, b'a', b':']), 0..8);
    prop::collection::vec(part, 0..5)
}

/// The same bytes split two ways: each mask marks where a part ends after a byte.
fn two_splits() -> impl Strategy<Value = (Vec<Vec<u8>>, Vec<Vec<u8>>)> {
    let bytes = prop::collection::vec(prop::sample::select(vec![0u8, b'a', b':']), 1..10);
    bytes.prop_flat_map(|bytes| {
        let mask = prop::collection::vec(any::<bool>(), bytes.len());
        (mask.clone(), mask)
            .prop_map(move |(left, right)| (split(&bytes, &left), split(&bytes, &right)))
    })
}

/// `bytes` cut after every byte whose `ends_part` is set, and at the end.
fn split(bytes: &[u8], ends_part: &[bool]) -> Vec<Vec<u8>> {
    let mut parts = vec![Vec::new()];
    for (byte, ends) in bytes.iter().zip(ends_part) {
        parts.last_mut().expect("one part at least").push(*byte);
        if *ends {
            parts.push(Vec::new());
        }
    }
    parts
}

fn encode(parts: &[Vec<u8>]) -> Vec<u8> {
    let borrowed: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    let subject = CustodySubject::try_from(borrowed.as_slice()).expect("small parts encode");
    subject.as_ref().to_vec()
}

proptest! {
    #[test]
    fn distinct_part_lists_encode_distinctly(left in part_list(), right in part_list()) {
        prop_assume!(left != right);

        prop_assert_ne!(encode(&left), encode(&right));
    }

    #[test]
    fn the_same_bytes_split_differently_encode_distinctly((left, right) in two_splits()) {
        prop_assume!(left != right);

        prop_assert_ne!(encode(&left), encode(&right));
    }

    #[test]
    fn a_subject_and_a_message_encode_alike(parts in part_list()) {
        let borrowed: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        let message = MacMessage::try_from(borrowed.as_slice()).expect("small parts encode");

        let subject_encoding = encode(&parts);
        prop_assert_eq!(message.as_ref(), subject_encoding.as_slice());
    }
}
