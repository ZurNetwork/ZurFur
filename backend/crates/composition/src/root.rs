//! The custody root: 32 bytes read from the environment by a command that holds a
//! purpose, and consumed by the one custody function. A migrating boot hands a zeroizing
//! copy of them to the custody migrator, which keeps it until its run returns or it is
//! dropped.

use std::env::VarError;
use std::fmt;
use std::marker::PhantomData;

use base64::DecodeSliceError;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use zeroize::Zeroizing;

/// The environment variable the custody root is read from: base64 of 32 bytes.
pub const ROOT_KEY_ENV: &str = "ZURFUR_DID_KEY_ROOT_KEY";

/// The custody root's length, in bytes.
const ROOT_BYTES: usize = 32;

/// The custody root: wiped on drop, never cloned, never printed, and bound to its
/// thread, so nothing `Send` (a `Runtime`) can hold it.
///
/// ```
/// let root = composition::RootSecret::from(zeroize::Zeroizing::new([7u8; 32]));
/// drop(root);
/// ```
///
/// It can't be copied:
///
/// ```compile_fail
/// let root = composition::RootSecret::from(zeroize::Zeroizing::new([7u8; 32]));
/// let copy: composition::RootSecret = root.clone();
/// ```
///
/// It can't cross to another thread:
///
/// ```compile_fail
/// let root = composition::RootSecret::from(zeroize::Zeroizing::new([7u8; 32]));
/// std::thread::spawn(move || drop(root));
/// ```
///
/// Nor can it be shared between threads:
///
/// ```compile_fail
/// fn shared<T: Sync>() {}
/// shared::<composition::RootSecret>();
/// ```
pub struct RootSecret {
    bytes: Zeroizing<[u8; ROOT_BYTES]>,
    thread_bound: PhantomData<*const ()>,
}

impl RootSecret {
    /// Reads and decodes [`ROOT_KEY_ENV`]; the error names the variable and the rule only.
    pub fn from_env() -> Result<Self, RootSecretError> {
        let encoded = match std::env::var(ROOT_KEY_ENV) {
            Ok(text) => Zeroizing::new(text),
            Err(VarError::NotPresent) => return Err(RootSecretError::Unset),
            Err(VarError::NotUnicode(raw)) => {
                let _wiped = Zeroizing::new(raw.into_encoded_bytes());
                return Err(RootSecretError::NotBase64);
            }
        };
        Self::try_from(encoded)
    }

    /// The root's bytes, for the one custody function; the root is gone afterwards.
    pub(crate) fn into_bytes(self) -> Zeroizing<[u8; ROOT_BYTES]> {
        self.bytes
    }
}

impl From<Zeroizing<[u8; ROOT_BYTES]>> for RootSecret {
    fn from(bytes: Zeroizing<[u8; ROOT_BYTES]>) -> Self {
        Self {
            bytes,
            thread_bound: PhantomData,
        }
    }
}

impl TryFrom<Zeroizing<String>> for RootSecret {
    type Error = RootSecretError;

    /// Decodes trimmed base64 into a wiped buffer; anything but 32 bytes is refused.
    fn try_from(encoded: Zeroizing<String>) -> Result<Self, Self::Error> {
        let mut bytes = Zeroizing::new([0u8; ROOT_BYTES]);
        let written = STANDARD
            .decode_slice(encoded.trim(), bytes.as_mut_slice())
            .map_err(decode_refusal)?;
        if written != ROOT_BYTES {
            return Err(RootSecretError::NotThirtyTwoBytes);
        }
        Ok(Self::from(bytes))
    }
}

impl fmt::Debug for RootSecret {
    /// Prints the type's name only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootSecret").finish_non_exhaustive()
    }
}

/// Why the custody root wasn't read. Names the variable and the rule, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RootSecretError {
    /// The variable isn't set.
    #[error("{ROOT_KEY_ENV} is not set")]
    Unset,
    /// The value isn't base64.
    #[error("{ROOT_KEY_ENV} is not base64")]
    NotBase64,
    /// The value decodes to something other than 32 bytes.
    #[error("{ROOT_KEY_ENV} is not 32 bytes")]
    NotThirtyTwoBytes,
}

/// The refusal for a decoder error, dropping the error itself: it names the offending byte.
fn decode_refusal(error: DecodeSliceError) -> RootSecretError {
    match error {
        DecodeSliceError::DecodeError(_) => RootSecretError::NotBase64,
        DecodeSliceError::OutputSliceTooSmall => RootSecretError::NotThirtyTwoBytes,
    }
}

#[cfg(test)]
mod tests;
