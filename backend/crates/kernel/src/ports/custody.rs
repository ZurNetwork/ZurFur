use std::{fmt, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::elements::custody::{
    CustodyError, CustodySubject, MacMessage, MacTag, MacUse, OpenedSecret, Purpose, SealPurpose,
    SealPurposeMarker, SealedEnvelope,
};

/// The custody of every key: seals and opens secrets bound to a subject, and tags and
/// verifies messages, each under the one key of its purpose. Held whole by composition
/// only; everything else holds a [`ScopedSealer`] or a [`ScopedMac`].
#[async_trait]
pub trait KeyCustodian: Send + Sync {
    /// Seal `plaintext` under `purpose`'s key, bound to `subject`.
    async fn seal(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError>;

    /// Open `envelope`, sealed under `purpose`'s key and bound to `subject`.
    async fn open(
        &self,
        purpose: SealPurpose,
        subject: &CustodySubject,
        envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError>;

    /// The tag of `message` for `mac_use`, under the Den's MAC key.
    async fn mac(&self, mac_use: MacUse, message: &MacMessage) -> Result<MacTag, CustodyError>;

    /// Check `tag` against `message` for `mac_use`; [`CustodyError::Unauthentic`] when it
    /// doesn't match.
    async fn verify(
        &self,
        mac_use: MacUse,
        message: &MacMessage,
        tag: &MacTag,
    ) -> Result<(), CustodyError>;
}

/// A custodian narrowed to one seal purpose, named by its type: a handle for one purpose
/// can't be passed where another's is wanted.
pub struct ScopedSealer<P> {
    custodian: Arc<dyn KeyCustodian>,
    purpose: PhantomData<fn() -> P>,
}

impl<P: SealPurposeMarker> ScopedSealer<P> {
    /// Seal `plaintext` under this handle's purpose, bound to `subject`.
    pub async fn seal(
        &self,
        subject: &CustodySubject,
        plaintext: &[u8],
    ) -> Result<SealedEnvelope, CustodyError> {
        self.custodian.seal(P::PURPOSE, subject, plaintext).await
    }

    /// Open `envelope` under this handle's purpose, bound to `subject`.
    pub async fn open(
        &self,
        subject: &CustodySubject,
        envelope: &SealedEnvelope,
    ) -> Result<OpenedSecret, CustodyError> {
        self.custodian.open(P::PURPOSE, subject, envelope).await
    }
}

impl<P: SealPurposeMarker> From<Arc<dyn KeyCustodian>> for ScopedSealer<P> {
    fn from(custodian: Arc<dyn KeyCustodian>) -> Self {
        Self {
            custodian,
            purpose: PhantomData,
        }
    }
}

impl<P> Clone for ScopedSealer<P> {
    /// Shares the custodian; no key is copied.
    fn clone(&self) -> Self {
        Self {
            custodian: Arc::clone(&self.custodian),
            purpose: PhantomData,
        }
    }
}

impl<P: SealPurposeMarker> fmt::Debug for ScopedSealer<P> {
    /// Prints the purpose only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedSealer")
            .field("purpose", &P::PURPOSE)
            .finish()
    }
}

/// A custodian narrowed to the Den's MAC key.
pub struct ScopedMac {
    custodian: Arc<dyn KeyCustodian>,
}

impl ScopedMac {
    /// The purpose whose key this handle uses.
    const PURPOSE: Purpose = Purpose::DenMac;

    /// The tag of `message` for `mac_use`.
    pub async fn mac(&self, mac_use: MacUse, message: &MacMessage) -> Result<MacTag, CustodyError> {
        self.custodian.mac(mac_use, message).await
    }

    /// Check `tag` against `message` for `mac_use`.
    pub async fn verify(
        &self,
        mac_use: MacUse,
        message: &MacMessage,
        tag: &MacTag,
    ) -> Result<(), CustodyError> {
        self.custodian.verify(mac_use, message, tag).await
    }
}

impl From<Arc<dyn KeyCustodian>> for ScopedMac {
    fn from(custodian: Arc<dyn KeyCustodian>) -> Self {
        Self { custodian }
    }
}

impl Clone for ScopedMac {
    /// Shares the custodian; no key is copied.
    fn clone(&self) -> Self {
        Self {
            custodian: Arc::clone(&self.custodian),
        }
    }
}

impl fmt::Debug for ScopedMac {
    /// Prints the purpose only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedMac")
            .field("purpose", &Self::PURPOSE)
            .finish()
    }
}

#[cfg(test)]
mod tests;
