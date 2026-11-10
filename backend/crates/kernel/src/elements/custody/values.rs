use std::fmt;

use zeroize::Zeroizing;

use super::Purpose;

/// The longest key id, in bytes.
const MAX_KEY_ID_BYTES: usize = 255;

/// The version byte that starts a sealed envelope this crate can read the header of.
pub const ENVELOPE_VERSION: u8 = 0x01;

/// The length of a MAC tag, in bytes.
const MAC_TAG_BYTES: usize = 32;

/// The name of a custody key, such as `hkdf:did_custody:<16 hex>`: 1 to 255 bytes of
/// `[a-z0-9_:-]`. Its `Debug` prints only its purpose; `Display` prints it whole.
#[derive(Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct KeyId(String);

/// Why text is not a [`KeyId`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeyIdError {
    /// The text is empty.
    #[error("a key id is not empty")]
    Empty,
    /// The text is longer than 255 bytes.
    #[error("a key id is at most 255 bytes")]
    TooLong,
    /// The text holds a byte outside `[a-z0-9_:-]`.
    #[error("a key id holds only a-z, 0-9, _, : and -")]
    OutsideAlphabet,
}

impl KeyId {
    /// The purpose named between the first two colons; `None` when there is no such
    /// token or it names no purpose. Logs at info and above print this, never the id.
    pub fn purpose(&self) -> Option<Purpose> {
        let mut tokens = self.0.splitn(3, ':');
        let _scheme = tokens.next()?;
        let purpose = tokens.next()?;
        let _fingerprint = tokens.next()?;
        purpose.parse().ok()
    }
}

impl TryFrom<String> for KeyId {
    type Error = KeyIdError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.is_empty() {
            return Err(KeyIdError::Empty);
        }
        if text.len() > MAX_KEY_ID_BYTES {
            return Err(KeyIdError::TooLong);
        }
        let in_alphabet = text
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b':' | b'-'));
        if !in_alphabet {
            return Err(KeyIdError::OutsideAlphabet);
        }
        Ok(Self(text))
    }
}

impl TryFrom<&str> for KeyId {
    type Error = KeyIdError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::try_from(text.to_owned())
    }
}

impl fmt::Debug for KeyId {
    /// Prints the purpose only: the rest of the id is a fingerprint of the root.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyId")
            .field("purpose", &self.purpose())
            .finish_non_exhaustive()
    }
}

/// Why a list of parts can't be encoded: a part, or the count, doesn't fit in a u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PartsError {
    /// The count of parts doesn't fit in a u32.
    #[error("too many parts to encode")]
    TooManyParts,
    /// A part's length doesn't fit in a u32.
    #[error("a part is too long to encode")]
    PartTooLong,
}

/// What a sealed secret is bound to, built from parts. Its encoding is the part count
/// as a big-endian u32, then each part as a big-endian u32 length and its bytes.
#[derive(Clone, PartialEq, Eq, derive_more::AsRef)]
#[as_ref([u8])]
pub struct CustodySubject(Vec<u8>);

impl TryFrom<&[&[u8]]> for CustodySubject {
    type Error = PartsError;

    fn try_from(parts: &[&[u8]]) -> Result<Self, Self::Error> {
        encode_parts(parts).map(Self)
    }
}

impl fmt::Debug for CustodySubject {
    /// Prints the encoding's length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustodySubject")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// What a MAC is computed over, built from parts and encoded as a [`CustodySubject`] is.
#[derive(Clone, PartialEq, Eq, derive_more::AsRef)]
#[as_ref([u8])]
pub struct MacMessage(Vec<u8>);

impl TryFrom<&[&[u8]]> for MacMessage {
    type Error = PartsError;

    fn try_from(parts: &[&[u8]]) -> Result<Self, Self::Error> {
        encode_parts(parts).map(Self)
    }
}

impl fmt::Debug for MacMessage {
    /// Prints the encoding's length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacMessage")
            .field("bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

/// The count of `parts`, then each part's length and bytes, every number a big-endian u32.
fn encode_parts(parts: &[&[u8]]) -> Result<Vec<u8>, PartsError> {
    let count = length_prefix(parts.len()).map_err(|_| PartsError::TooManyParts)?;
    let mut encoding = count.to_vec();
    for part in parts {
        let length = length_prefix(part.len()).map_err(|_| PartsError::PartTooLong)?;
        encoding.extend_from_slice(&length);
        encoding.extend_from_slice(part);
    }
    Ok(encoding)
}

/// `length` as a big-endian u32; refused when it doesn't fit, never truncated.
fn length_prefix(length: usize) -> Result<[u8; 4], std::num::TryFromIntError> {
    u32::try_from(length).map(u32::to_be_bytes)
}

/// A sealed secret as it is stored. Its `Debug` prints the length and the purpose its
/// header names, nothing else.
#[derive(Clone, PartialEq, Eq, derive_more::From, derive_more::AsRef)]
#[as_ref([u8])]
pub struct SealedEnvelope(Vec<u8>);

impl SealedEnvelope {
    /// The purpose of the key id in a version-1 header (the version byte, the key id's
    /// length byte, the key id); `None` when the header doesn't parse.
    fn header_purpose(&self) -> Option<Purpose> {
        let [version, key_id_length, rest @ ..] = self.0.as_slice() else {
            return None;
        };
        if *version != ENVELOPE_VERSION {
            return None;
        }
        let key_id_bytes = rest.get(..usize::from(*key_id_length))?;
        let key_id_text = std::str::from_utf8(key_id_bytes).ok()?;
        KeyId::try_from(key_id_text).ok()?.purpose()
    }
}

impl fmt::Debug for SealedEnvelope {
    /// Prints the length and the header's purpose only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedEnvelope")
            .field("bytes", &self.0.len())
            .field("purpose", &self.header_purpose())
            .finish()
    }
}

/// A MAC tag and the key that made it. Neither comparable nor cloneable: a tag is checked
/// only by the custodian, in constant time.
pub struct MacTag {
    key_id: KeyId,
    tag: [u8; MAC_TAG_BYTES],
}

/// Why bytes are not a [`MacTag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MacTagError {
    /// The bytes are `length` long, not 32.
    #[error("a MAC tag is 32 bytes, not {length}")]
    WrongLength { length: usize },
}

impl MacTag {
    /// The key that made the tag, for storage.
    pub fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    /// The tag's bytes, for storage.
    pub fn stored_bytes(&self) -> &[u8] {
        &self.tag
    }
}

impl TryFrom<(KeyId, &[u8])> for MacTag {
    type Error = MacTagError;

    fn try_from((key_id, bytes): (KeyId, &[u8])) -> Result<Self, Self::Error> {
        let tag = <[u8; MAC_TAG_BYTES]>::try_from(bytes).map_err(|_| MacTagError::WrongLength {
            length: bytes.len(),
        })?;
        Ok(Self { key_id, tag })
    }
}

impl fmt::Debug for MacTag {
    /// Prints the key's purpose and the tag's length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacTag")
            .field("purpose", &self.key_id.purpose())
            .field("tag_bytes", &self.tag.len())
            .finish()
    }
}

/// An opened secret, wiped from memory on drop. Neither comparable nor cloneable, and its
/// `Debug` prints nothing of it.
#[derive(derive_more::From)]
#[from(Vec<u8>)]
pub struct OpenedSecret(Zeroizing<Vec<u8>>);

impl OpenedSecret {
    /// The secret's bytes, for the one use that needs them.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for OpenedSecret {
    /// Prints nothing of the secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedSecret").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod proptests;
