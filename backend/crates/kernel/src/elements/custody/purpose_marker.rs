use super::SealPurpose;

/// The seal purpose a handle's type names. Only this crate's markers implement it.
pub trait SealPurposeMarker: sealed::Sealed {
    /// The purpose a handle of this marker passes on every call.
    const PURPOSE: SealPurpose;
}

/// The purpose of sealing the did:plc custody keys. A type-level marker, never a value.
pub enum ForDidCustody {}

/// The purpose of sealing sign-in's OAuth secrets. A type-level marker, never a value.
pub enum ForSigninSecrets {}

impl SealPurposeMarker for ForDidCustody {
    const PURPOSE: SealPurpose = SealPurpose::DidCustody;
}

impl SealPurposeMarker for ForSigninSecrets {
    const PURPOSE: SealPurpose = SealPurpose::SigninSecrets;
}

/// The private supertrait that keeps every other crate from adding a marker.
mod sealed {
    /// Implemented by this crate's purpose markers only.
    pub trait Sealed {}

    impl Sealed for super::ForDidCustody {}

    impl Sealed for super::ForSigninSecrets {}
}

#[cfg(test)]
mod tests;
