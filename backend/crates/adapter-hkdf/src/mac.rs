use domain::elements::custody::{CustodyError, KeyId, MacMessage, MacTag, MacUse};
use hmac::{Hmac, KeyInit, Mac};
use sha2_0_11::Sha256;

use crate::derivation::KEY_BYTES;
use crate::envelope::push_length_prefixed;

/// The label a MAC's input starts with, before the use's token.
pub(crate) const MAC_LABEL_PREFIX: &[u8] = b"zurfur/mac/v1/";

/// The length of a MAC tag, in bytes.
pub(crate) const MAC_TAG_BYTES: usize = 32;

/// The Den's MAC key: its id and one HMAC-SHA256 keyed once, cloned for every tag so
/// the padded key block is never rebuilt.
pub(crate) struct DenMacKey {
    key_id: KeyId,
    keyed: Hmac<Sha256>,
}

impl From<(KeyId, &[u8; KEY_BYTES])> for DenMacKey {
    fn from((key_id, key): (KeyId, &[u8; KEY_BYTES])) -> Self {
        let keyed = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
            .expect("HMAC accepts a key of any length");
        Self { key_id, keyed }
    }
}

impl DenMacKey {
    /// The tag of `message` for `mac_use`, named by this key's id.
    pub(crate) fn tag(
        &self,
        mac_use: MacUse,
        message: &MacMessage,
    ) -> Result<MacTag, CustodyError> {
        let mut hmac = self.keyed.clone();
        hmac.update(&mac_input(mac_use, message)?);
        let tag: [u8; MAC_TAG_BYTES] = hmac.finalize().into_bytes().into();
        MacTag::try_from((self.key_id.clone(), tag.as_slice())).map_err(|_| CustodyError::Malformed)
    }

    /// Check `tag` in constant time; [`CustodyError::UnknownKeyId`] when another key
    /// made it, [`CustodyError::Unauthentic`] when it doesn't match.
    pub(crate) fn verify(
        &self,
        mac_use: MacUse,
        message: &MacMessage,
        tag: &MacTag,
    ) -> Result<(), CustodyError> {
        if tag.key_id() != &self.key_id {
            let unknown_key_id = CustodyError::UnknownKeyId {
                key_id: tag.key_id().clone(),
            };
            return Err(unknown_key_id);
        }
        let mut hmac = self.keyed.clone();
        hmac.update(&mac_input(mac_use, message)?);
        hmac.verify_slice(tag.stored_bytes())
            .map_err(|_| CustodyError::Unauthentic)
    }
}

/// The length-prefixed label `zurfur/mac/v1/<use>`, then the message's encoding.
fn mac_input(mac_use: MacUse, message: &MacMessage) -> Result<Vec<u8>, CustodyError> {
    let mut input = Vec::new();
    push_length_prefixed(&mut input, &mac_label(mac_use))?;
    input.extend_from_slice(message.as_ref());
    Ok(input)
}

/// `zurfur/mac/v1/` followed by `mac_use`'s persisted token.
pub(crate) fn mac_label(mac_use: MacUse) -> Vec<u8> {
    let use_token: &'static str = mac_use.into();
    [MAC_LABEL_PREFIX, use_token.as_bytes()].concat()
}

#[cfg(test)]
mod tests;
