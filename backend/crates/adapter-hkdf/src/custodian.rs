use std::fmt;

use async_trait::async_trait;
use domain::elements::custody::{
    CustodyError, CustodySubject, KeyId, MacMessage, MacTag, MacUse, OpenedSecret, Purpose,
    SealPurpose, SealedEnvelope,
};
use domain::ports::KeyCustodian;
use zeroize::Zeroizing;

use crate::derivation::{self, KEY_BYTES};
use crate::envelope::{self, EnvelopeParts};
use crate::mac::DenMacKey;

/// The dev key custodian: holds one HKDF-derived key per purpose it was built with,
/// and nothing else: no root, no PRK, and no way to derive a purpose it lacks.
pub struct HkdfKeyCustodian {
    seal_keys: Vec<SealKey>,
    den_mac: Option<DenMacKey>,
}

/// A seal purpose's key and its id.
struct SealKey {
    purpose: SealPurpose,
    key_id: KeyId,
    key: Zeroizing<[u8; KEY_BYTES]>,
}

impl HkdfKeyCustodian {
    /// Derives the key of each of `purposes` from `root` at once, then drops the root.
    /// Named, not `From`: it runs the derivation, which a conversion would hide.
    pub fn derive(root: Zeroizing<[u8; KEY_BYTES]>, purposes: &[Purpose]) -> Self {
        let mut seal_keys = Vec::new();
        let mut den_mac = None;
        let mut derived = Vec::new();
        for &purpose in purposes {
            if derived.contains(&purpose) {
                continue;
            }
            derived.push(purpose);
            let key = derivation::purpose_key(&root, purpose);
            let key_id = derivation::key_id(&root, purpose);
            match purpose {
                Purpose::DidCustody => seal_keys.push(SealKey {
                    purpose: SealPurpose::DidCustody,
                    key_id,
                    key,
                }),
                Purpose::SigninSecrets => seal_keys.push(SealKey {
                    purpose: SealPurpose::SigninSecrets,
                    key_id,
                    key,
                }),
                Purpose::DenMac => den_mac = Some(DenMacKey::from((key_id, &*key))),
            }
        }
        Self { seal_keys, den_mac }
    }

    /// The purposes this custodian holds a key for.
    fn held_purposes(&self) -> Vec<Purpose> {
        let seal_purposes = self
            .seal_keys
            .iter()
            .map(|seal_key| Purpose::from(seal_key.purpose));
        let den_mac = self.den_mac.as_ref().map(|_| Purpose::DenMac);
        seal_purposes.chain(den_mac).collect()
    }

    /// The key of `purpose`; [`CustodyError::PurposeNotHeld`] when it wasn't built with it.
    fn seal_key(&self, purpose: SealPurpose) -> Result<&SealKey, CustodyError> {
        self.seal_keys
            .iter()
            .find(|seal_key| seal_key.purpose == purpose)
            .ok_or_else(|| CustodyError::PurposeNotHeld {
                purpose: Purpose::from(purpose),
            })
    }

    /// The Den's MAC key; [`CustodyError::PurposeNotHeld`] when it wasn't built with it.
    fn den_mac_key(&self) -> Result<&DenMacKey, CustodyError> {
        self.den_mac.as_ref().ok_or(CustodyError::PurposeNotHeld {
            purpose: Purpose::DenMac,
        })
    }

    /// Seals under the purpose's key with a fresh nonce.
    fn seal_sync(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        let seal_key = self.seal_key(purpose)?;
        envelope::seal(&seal_key.key, &seal_key.key_id, purpose, subject, plaintext)
    }

    /// Reads the version, then the key id, which must be the purpose's own, then opens.
    fn open_sync(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        sealed: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        let seal_key = self.seal_key(purpose)?;
        let parts = EnvelopeParts::try_from(sealed.as_ref())?;
        if parts.key_id() != &seal_key.key_id {
            let unknown_key_id = CustodyError::UnknownKeyId {
                key_id: parts.key_id().clone(),
            };
            return Err(unknown_key_id);
        }
        envelope::open(&parts, &seal_key.key, purpose, subject)
    }
}

#[async_trait]
impl KeyCustodian for HkdfKeyCustodian {
    async fn seal(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        self.seal_sync(purpose, subject, plaintext)
    }

    async fn open(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        self.open_sync(purpose, subject, envelope)
    }

    async fn mac(&self, mac_use: MacUse, message: &MacMessage) -> Result<MacTag, CustodyError> {
        self.den_mac_key()?.tag(mac_use, message)
    }

    async fn verify(
        &self,
        mac_use: MacUse,
        message: &MacMessage,
        tag: &MacTag,
    ) -> Result<(), CustodyError> {
        self.den_mac_key()?.verify(mac_use, message, tag)
    }
}

impl fmt::Debug for HkdfKeyCustodian {
    /// Prints the held purposes only: never a key, a key id or the root.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HkdfKeyCustodian")
            .field("purposes", &self.held_purposes())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
