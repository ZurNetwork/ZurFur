//! A `MacTag` can't be cloned.

use kernel::elements::custody::{KeyId, MacTag};

fn main() {
    let key_id = KeyId::try_from("hkdf:den_mac:0123456789abcdef").unwrap();
    let tag = MacTag::try_from((key_id, [0u8; 32].as_slice())).unwrap();
    let _copy = tag.clone();
}
