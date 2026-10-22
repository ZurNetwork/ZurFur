//! Key custody's vocabulary: the purposes a custody key serves ([`Purpose`],
//! [`SealPurpose`], [`MacUse`]), the type-level purpose markers ([`ForDidCustody`],
//! [`ForSigninSecrets`]), the values a custodian takes and returns, and [`CustodyError`].
//!
//! None of these values prints a secret, a subject, a tag or a full key id.

mod errors;
mod purpose;
mod purpose_marker;
mod values;

pub use errors::CustodyError;
pub use purpose::{
    MacUse, Purpose, SealPurpose, UnknownMacUse, UnknownPurpose, UnknownSealPurpose,
};
pub use purpose_marker::{ForDidCustody, ForSigninSecrets, SealPurposeMarker};
pub use values::{
    CustodySubject, ENVELOPE_VERSION, KeyId, KeyIdError, MacMessage, MacTag, MacTagError,
    OpenedSecret, PartsError, SealedEnvelope,
};
