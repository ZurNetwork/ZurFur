//! The golden file `golden/custody-v1.json`, read for the tests. Every expected byte
//! comes from it, computed by `golden/custody_v1.py`, never from the code under test.

use domain::elements::custody::{CustodySubject, MacMessage};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::derivation::KEY_BYTES;

/// The golden file's text, compiled in.
const GOLDEN_TEXT: &str = include_str!("../golden/custody-v1.json");

/// The golden file, parsed.
pub(crate) fn golden() -> Value {
    serde_json::from_str(GOLDEN_TEXT).expect("the golden file is JSON")
}

/// The text at `pointer` in `value`.
pub(crate) fn text_at<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("the golden file has text at {pointer}"))
}

/// The number at `pointer` in `value`.
pub(crate) fn number_at(value: &Value, pointer: &str) -> usize {
    let number = value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("the golden file has a number at {pointer}"));
    usize::try_from(number).expect("a golden number fits in usize")
}

/// The array at `pointer` in `value`.
pub(crate) fn array_at<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("the golden file has an array at {pointer}"))
}

/// The bytes the lowercase hex at `pointer` in `value` spells.
pub(crate) fn bytes_at(value: &Value, pointer: &str) -> Vec<u8> {
    hex_bytes(text_at(value, pointer))
}

/// The bytes `hex` spells; panics on anything but lowercase hex.
pub(crate) fn hex_bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "golden hex has an odd length");
    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("golden hex is ASCII");
            let is_lowercase = digits
                .bytes()
                .all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f'));
            assert!(is_lowercase, "golden hex is lowercase");
            u8::from_str_radix(digits, 16).expect("golden hex parses")
        })
        .collect()
}

/// `bytes` as lowercase hex.
pub(crate) fn hex_text(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The golden file's test root.
pub(crate) fn test_root() -> Zeroizing<[u8; KEY_BYTES]> {
    let root_bytes = bytes_at(&golden(), "/root");
    let root = <[u8; KEY_BYTES]>::try_from(root_bytes.as_slice()).expect("the root is 32 bytes");
    Zeroizing::new(root)
}

/// The hex-encoded parts of the array `parts`, as bytes.
pub(crate) fn parts_of(parts: &[Value]) -> Vec<Vec<u8>> {
    parts
        .iter()
        .map(|part| hex_bytes(part.as_str().expect("a golden part is hex text")))
        .collect()
}

/// The golden subject named `name`, built by kernel from its parts.
pub(crate) fn subject(name: &str) -> CustodySubject {
    let golden = golden();
    let parts = parts_of(array_at(&golden, &format!("/subjects/{name}/parts")));
    let part_slices: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    CustodySubject::try_from(part_slices.as_slice()).expect("a golden subject encodes")
}

/// The golden MAC message at `index`, built by kernel from its parts.
pub(crate) fn mac_message(index: usize) -> MacMessage {
    let golden = golden();
    let parts = parts_of(array_at(&golden, &format!("/mac_messages/{index}/parts")));
    let part_slices: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    MacMessage::try_from(part_slices.as_slice()).expect("a golden message encodes")
}
