//! A `MacTag` can't be compared with `==`: a tag is checked only by the custodian, in
//! constant time.

use kernel::elements::custody::{KeyId, MacTag};

fn main() {
    let key_id = KeyId::try_from("hkdf:den_mac:0123456789abcdef").unwrap();
    let tag = MacTag::try_from((key_id.clone(), [0u8; 32].as_slice())).unwrap();
    let other = MacTag::try_from((key_id, [0u8; 32].as_slice())).unwrap();
    let _same = tag == other;
}
