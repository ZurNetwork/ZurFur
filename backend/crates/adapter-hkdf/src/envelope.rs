use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::elements::custody::{
    CustodyError, CustodySubject, ENVELOPE_VERSION, KeyId, OpenedSecret, Purpose, SealPurpose,
    SealedEnvelope,
};

use crate::derivation::KEY_BYTES;

/// The length of an envelope's random nonce, in bytes.
pub(crate) const NONCE_BYTES: usize = 24;

/// The length of the AEAD tag that ends an envelope, in bytes.
pub(crate) const AEAD_TAG_BYTES: usize = 16;

/// A v1 envelope read into its parts: the key id, the nonce, and the ciphertext with
/// its tag. Nothing in it is authenticated yet.
pub(crate) struct EnvelopeParts<'a> {
    key_id: KeyId,
    nonce: XNonce,
    sealed: &'a [u8],
}

impl EnvelopeParts<'_> {
    /// The key id the header names; checked against the alphabet, not yet authentic.
    pub(crate) fn key_id(&self) -> &KeyId {
        &self.key_id
    }
}

impl<'a> TryFrom<&'a [u8]> for EnvelopeParts<'a> {
    type Error = CustodyError;

    /// Reads the version, then the key id, then the nonce and the sealed body; never panics.
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let [version, rest @ ..] = bytes else {
            return Err(CustodyError::Malformed);
        };
        if *version != ENVELOPE_VERSION {
            return Err(CustodyError::UnknownVersion);
        }
        let [key_id_length, rest @ ..] = rest else {
            return Err(CustodyError::Malformed);
        };
        let (key_id_bytes, rest) = rest
            .split_at_checked(usize::from(*key_id_length))
            .ok_or(CustodyError::Malformed)?;
        let key_id_text = std::str::from_utf8(key_id_bytes).map_err(|_| CustodyError::Malformed)?;
        let key_id = KeyId::try_from(key_id_text).map_err(|_| CustodyError::Malformed)?;
        let (nonce, sealed) = rest
            .split_at_checked(NONCE_BYTES)
            .ok_or(CustodyError::Malformed)?;
        if sealed.len() < AEAD_TAG_BYTES {
            return Err(CustodyError::Malformed);
        }
        let nonce = XNonce::clone_from_slice(nonce);
        Ok(Self {
            key_id,
            nonce,
            sealed,
        })
    }
}

/// Seal `plaintext` under `key` with a fresh random nonce, bound to `purpose`, `key_id`
/// and `subject`.
pub(crate) fn seal(
    key: &[u8; KEY_BYTES],
    key_id: &KeyId,
    purpose: SealPurpose,
    subject: &CustodySubject,
    plaintext: &[u8],
) -> Result<SealedEnvelope, CustodyError> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    seal_with_nonce(key, key_id, purpose, subject, &nonce, plaintext)
}

/// Seal under a given nonce: `seal`'s body, and the golden tests' way to reproduce an
/// envelope byte for byte.
fn seal_with_nonce(
    key: &[u8; KEY_BYTES],
    key_id: &KeyId,
    purpose: SealPurpose,
    subject: &CustodySubject,
    nonce: &XNonce,
    plaintext: &[u8],
) -> Result<SealedEnvelope, CustodyError> {
    let associated_data = associated_data(purpose, key_id, subject)?;
    let payload = Payload {
        msg: plaintext,
        aad: &associated_data,
    };
    let ciphertext = XChaCha20Poly1305::new(key.into())
        .encrypt(nonce, payload)
        .map_err(|_| CustodyError::Malformed)?;
    let key_id_bytes = key_id.as_ref().as_bytes();
    let key_id_length = u8::try_from(key_id_bytes.len()).map_err(|_| CustodyError::Malformed)?;
    let mut bytes = Vec::with_capacity(2 + key_id_bytes.len() + NONCE_BYTES + ciphertext.len());
    bytes.push(ENVELOPE_VERSION);
    bytes.push(key_id_length);
    bytes.extend_from_slice(key_id_bytes);
    bytes.extend_from_slice(nonce);
    bytes.extend_from_slice(&ciphertext);
    Ok(SealedEnvelope::from(bytes))
}

/// Open `parts` under `key`; [`CustodyError::Unauthentic`] unless they were sealed
/// under it for `purpose`, their key id and `subject`, unaltered.
pub(crate) fn open(
    parts: &EnvelopeParts<'_>,
    key: &[u8; KEY_BYTES],
    purpose: SealPurpose,
    subject: &CustodySubject,
) -> Result<OpenedSecret, CustodyError> {
    let associated_data = associated_data(purpose, &parts.key_id, subject)?;
    let payload = Payload {
        msg: parts.sealed,
        aad: &associated_data,
    };
    let plaintext = XChaCha20Poly1305::new(key.into())
        .decrypt(&parts.nonce, payload)
        .map_err(|_| CustodyError::Unauthentic)?;
    Ok(OpenedSecret::from(plaintext))
}

/// The version, then the purpose's token, the key id and the subject's encoding, each
/// length-prefixed.
fn associated_data(
    purpose: SealPurpose,
    key_id: &KeyId,
    subject: &CustodySubject,
) -> Result<Vec<u8>, CustodyError> {
    let purpose_token: &'static str = Purpose::from(purpose).into();
    associated_data_of_fields(
        purpose_token.as_bytes(),
        key_id.as_ref().as_bytes(),
        subject.as_ref(),
    )
}

/// The version byte, then each field as a big-endian u32 length and its bytes.
fn associated_data_of_fields(
    purpose_token: &[u8],
    key_id: &[u8],
    subject: &[u8],
) -> Result<Vec<u8>, CustodyError> {
    let mut associated_data = vec![ENVELOPE_VERSION];
    push_length_prefixed(&mut associated_data, purpose_token)?;
    push_length_prefixed(&mut associated_data, key_id)?;
    push_length_prefixed(&mut associated_data, subject)?;
    Ok(associated_data)
}

/// Append `field`'s length as a big-endian u32, then `field`; refused when the length
/// doesn't fit in a u32.
pub(crate) fn push_length_prefixed(out: &mut Vec<u8>, field: &[u8]) -> Result<(), CustodyError> {
    let length = u32::try_from(field.len()).map_err(|_| CustodyError::Malformed)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(field);
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod proptests;
