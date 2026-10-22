//! The one custody function: the boot guard on the raw root, then the key
//! derivation, then the purpose-scoped handles the adapters hold.

use std::sync::Arc;

use adapter_hkdf::HkdfKeyCustodian;
use domain::elements::custody::{ForDidCustody, ForSigninSecrets, Purpose};
use domain::ports::{KeyCustodian, ScopedSealer};
use zeroize::Zeroizing;

use crate::{Environment, ensure_custody_hardened};

/// The length of the custody root, in bytes.
const ROOT_BYTES: usize = 32;

/// The purposes whose keys the root is derived into.
const SEAL_PURPOSES: [Purpose; 2] = [Purpose::DidCustody, Purpose::SigninSecrets];

/// The sealing handles the adapters hold, one per purpose, over one custodian.
#[derive(Debug)]
pub(crate) struct SealHandles {
    /// Seals the did:plc custody keys, for `PgKeyStore`.
    pub(crate) did_custody: ScopedSealer<ForDidCustody>,
    /// Seals sign-in's OAuth secrets, for the atproto auth store.
    pub(crate) signin_secrets: ScopedSealer<ForSigninSecrets>,
}

/// Guards `root_bytes` for `env`, derives both seal purposes' keys from them once
/// and returns their handles. Refuses a root that isn't 32 bytes, naming its length only.
pub(crate) fn seal_handles(
    root_bytes: &[u8],
    env: &Environment,
    plc_directory_submit: bool,
) -> anyhow::Result<SealHandles> {
    ensure_custody_hardened(env, root_bytes, plc_directory_submit)?;
    if root_bytes.len() != ROOT_BYTES {
        anyhow::bail!(
            "the custody root key must be exactly {ROOT_BYTES} bytes, got {}",
            root_bytes.len()
        );
    }
    let mut root = Zeroizing::new([0u8; ROOT_BYTES]);
    root.copy_from_slice(root_bytes);
    let derived = HkdfKeyCustodian::derive(root, &SEAL_PURPOSES);
    let custodian: Arc<dyn KeyCustodian> = Arc::new(derived);
    let handles = SealHandles {
        did_custody: ScopedSealer::from(Arc::clone(&custodian)),
        signin_secrets: ScopedSealer::from(custodian),
    };
    Ok(handles)
}

#[cfg(test)]
mod tests;
