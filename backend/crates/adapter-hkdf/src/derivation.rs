use domain::elements::custody::{KeyId, Purpose};
use hkdf::{Hkdf, InvalidLength};
use sha2_0_11::Sha256;
use zeroize::{Zeroize, Zeroizing};

/// The HKDF salt of every custody derivation.
pub(crate) const SALT: &[u8] = b"zurfur/custody/v1";

/// The HKDF info of a purpose key, before the purpose's token.
pub(crate) const KEY_INFO_PREFIX: &[u8] = b"zurfur/custody/v1/key/";

/// The HKDF info of a key id's fingerprint, before the purpose's token.
pub(crate) const KEY_ID_INFO_PREFIX: &[u8] = b"zurfur/custody/v1/key-id/";

/// The length of a purpose key, in bytes.
pub(crate) const KEY_BYTES: usize = 32;

/// The length of a key id's fingerprint, in bytes.
pub(crate) const FINGERPRINT_BYTES: usize = 8;

/// The scheme that starts every key id this crate names.
const KEY_ID_SCHEME: &str = "hkdf";

/// HKDF-SHA256: Extract `ikm` under `salt`, wipe the PRK, then Expand `info` into
/// `out`. Fails only when `out` is longer than HKDF-SHA256 can expand.
pub(crate) fn derive(
    salt: &[u8],
    ikm: &[u8],
    info: &[u8],
    out: &mut [u8],
) -> Result<(), InvalidLength> {
    let (mut prk, hkdf) = Hkdf::<Sha256>::extract(Some(salt), ikm);
    prk.as_mut_slice().zeroize();
    hkdf.expand(info, out)
}

/// The key `root` derives for `purpose`.
pub(crate) fn purpose_key(root: &[u8; KEY_BYTES], purpose: Purpose) -> Zeroizing<[u8; KEY_BYTES]> {
    let info = info_label(KEY_INFO_PREFIX, purpose);
    let mut key = Zeroizing::new([0u8; KEY_BYTES]);
    derive(SALT, root, &info, key.as_mut_slice())
        .expect("a 32-byte key is within HKDF-SHA256's output limit");
    key
}

/// The key id of `purpose`'s key under `root`: `hkdf:<purpose>:<16 lowercase hex>`.
pub(crate) fn key_id(root: &[u8; KEY_BYTES], purpose: Purpose) -> KeyId {
    let info = info_label(KEY_ID_INFO_PREFIX, purpose);
    let mut fingerprint = [0u8; FINGERPRINT_BYTES];
    derive(SALT, root, &info, &mut fingerprint)
        .expect("an 8-byte fingerprint is within HKDF-SHA256's output limit");
    let fingerprint_hex: String = fingerprint
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let token: &'static str = purpose.into();
    let key_id_text = format!("{KEY_ID_SCHEME}:{token}:{fingerprint_hex}");
    KeyId::try_from(key_id_text).expect("a derived key id is inside the key-id alphabet")
}

/// `prefix` followed by `purpose`'s persisted token.
pub(crate) fn info_label(prefix: &[u8], purpose: Purpose) -> Vec<u8> {
    let token: &'static str = purpose.into();
    [prefix, token.as_bytes()].concat()
}

#[cfg(test)]
mod tests;
