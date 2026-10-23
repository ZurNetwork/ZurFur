//! The one custody function: the boot guard on the raw root, then the key derivation,
//! then the purpose-scoped handles the adapters hold. The only file that names the
//! custodian port or its HKDF adapter.

use std::sync::Arc;

use adapter_hkdf::HkdfKeyCustodian;
use adapter_pg::{PgKeyStore, PgPool, SealedRowCheck, SqlxError};
use async_trait::async_trait;
use domain::elements::custody::{
    CustodyError, CustodySubject, ForDidCustody, ForSigninSecrets, MacMessage, MacTag, MacUse,
    OpenedSecret, Purpose, SealPurpose, SealedEnvelope,
};
use domain::ports::{KeyCustodian, ScopedMac, ScopedSealer};
use zeroize::Zeroizing;

use crate::{Environment, RootSecret, ensure_custody_hardened};

/// The purpose-scoped handles of one custodian: a handle whose purpose it wasn't
/// derived with answers `PurposeNotHeld`.
#[derive(Debug)]
pub struct Custody {
    /// Seals the did:plc custody keys, for `PgKeyStore`.
    pub(crate) did_custody: ScopedSealer<ForDidCustody>,
    /// Seals sign-in's OAuth secrets, for the atproto authenticator.
    pub(crate) signin_secrets: ScopedSealer<ForSigninSecrets>,
    /// Tags and verifies the Den's records.
    pub(crate) den_mac: ScopedMac,
}

impl Custody {
    /// Guards `root` for `env` and `plc_directory_submit`, then derives the keys of
    /// `purposes` from it. The root is gone either way.
    pub fn derive(
        root: RootSecret,
        env: &Environment,
        plc_directory_submit: bool,
        purposes: &[Purpose],
    ) -> anyhow::Result<Self> {
        guard_then_derive(root.into_bytes(), env, plc_directory_submit, purposes)
    }

    /// The Den's MAC handle.
    pub fn den_mac(&self) -> &ScopedMac {
        &self.den_mac
    }

    /// The check that this custody's DID-custody root opens the custody rows, split off
    /// before [`Runtime::connect`](crate::Runtime::connect) takes the handles.
    pub fn root_check(&self) -> CustodyRootCheck {
        CustodyRootCheck {
            sealer: self.did_custody.clone(),
        }
    }
}

/// Checks that a command's DID-custody root opens the first sealed custody row. Holds a
/// copy of the DID-custody handle and lends it to nothing else.
#[derive(Debug)]
pub struct CustodyRootCheck {
    sealer: ScopedSealer<ForDidCustody>,
}

impl CustodyRootCheck {
    /// Opens the first sealed custody row on `pool` through the DID-custody handle;
    /// writes nothing.
    pub async fn run(&self, pool: &PgPool) -> Result<SealedRowCheck, SqlxError> {
        PgKeyStore::check_root(pool, &self.sealer).await
    }
}

impl Default for Custody {
    /// Holds no purpose and reads no root: every handle answers `PurposeNotHeld`.
    fn default() -> Self {
        handles(Arc::new(NoPurposeHeld))
    }
}

/// The custodian of a command that holds no purpose: it has no key and refuses every call.
struct NoPurposeHeld;

#[async_trait]
impl KeyCustodian for NoPurposeHeld {
    async fn seal(
        &self,
        purpose: SealPurpose,
        _subject: &CustodySubject,
        _plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        Err(not_held(Purpose::from(purpose)))
    }

    async fn open(
        &self,
        purpose: SealPurpose,
        _subject: &CustodySubject,
        _envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        Err(not_held(Purpose::from(purpose)))
    }

    async fn mac(&self, _mac_use: MacUse, _message: &MacMessage) -> Result<MacTag, CustodyError> {
        Err(not_held(Purpose::DenMac))
    }

    async fn verify(
        &self,
        _mac_use: MacUse,
        _message: &MacMessage,
        _tag: &MacTag,
    ) -> Result<(), CustodyError> {
        Err(not_held(Purpose::DenMac))
    }
}

/// The refusal for a purpose no key is held for.
fn not_held(purpose: Purpose) -> CustodyError {
    CustodyError::PurposeNotHeld { purpose }
}

/// [`Custody::derive`], also handing back a copy of the root for the custody migrator,
/// which opens the 0.1 rows sealed under it.
pub(crate) fn derive_keeping_legacy_root(
    root: RootSecret,
    env: &Environment,
    plc_directory_submit: bool,
    purposes: &[Purpose],
) -> anyhow::Result<(Custody, Zeroizing<[u8; 32]>)> {
    let root_bytes = root.into_bytes();
    let legacy_root = root_bytes.clone();
    let custody = guard_then_derive(root_bytes, env, plc_directory_submit, purposes)?;
    Ok((custody, legacy_root))
}

/// The one custody function: refuses `root` for `env` and `plc_directory_submit`, or
/// derives the keys of `purposes` from it and returns their handles.
fn guard_then_derive(
    root: Zeroizing<[u8; 32]>,
    env: &Environment,
    plc_directory_submit: bool,
    purposes: &[Purpose],
) -> anyhow::Result<Custody> {
    ensure_custody_hardened(env, root.as_slice(), plc_directory_submit)?;
    let derived = HkdfKeyCustodian::derive(root, purposes);
    Ok(handles(Arc::new(derived)))
}

/// Every purpose's handle over `custodian`.
fn handles(custodian: Arc<dyn KeyCustodian>) -> Custody {
    Custody {
        did_custody: ScopedSealer::from(Arc::clone(&custodian)),
        signin_secrets: ScopedSealer::from(Arc::clone(&custodian)),
        den_mac: ScopedMac::from(custodian),
    }
}

#[cfg(test)]
mod tests;
