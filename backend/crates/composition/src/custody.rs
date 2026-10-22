//! The one custody function: the boot guard on the raw root, then the key
//! derivation, then the purpose-scoped handles the adapters hold.

use std::sync::Arc;

use adapter_hkdf::HkdfKeyCustodian;
use domain::elements::custody::{ForDidCustody, Purpose};
use domain::ports::{KeyCustodian, ScopedSealer};
use zeroize::Zeroizing;

use crate::{Environment, ensure_custody_hardened};

/// The length of the custody root, in bytes.
const ROOT_BYTES: usize = 32;

/// Guards `root_bytes` for `env`, derives the DID-custody key from them and
/// returns its handle. Refuses a root that isn't 32 bytes, naming its length only.
pub(crate) fn did_custody_sealer(
    root_bytes: &[u8],
    env: &Environment,
    plc_directory_submit: bool,
) -> anyhow::Result<ScopedSealer<ForDidCustody>> {
    ensure_custody_hardened(env, root_bytes, plc_directory_submit)?;
    if root_bytes.len() != ROOT_BYTES {
        anyhow::bail!(
            "the custody root key must be exactly {ROOT_BYTES} bytes, got {}",
            root_bytes.len()
        );
    }
    let mut root = Zeroizing::new([0u8; ROOT_BYTES]);
    root.copy_from_slice(root_bytes);
    let derived = HkdfKeyCustodian::derive(root, &[Purpose::DidCustody]);
    let custodian: Arc<dyn KeyCustodian> = Arc::new(derived);
    Ok(ScopedSealer::from(custodian))
}

#[cfg(test)]
mod tests;
