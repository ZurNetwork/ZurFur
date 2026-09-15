use serde::{Deserialize, Serialize};

/// The app-private key of a [`Commission`] (UUIDv7, so it sorts by creation
/// time).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
#[serde(transparent)]
pub struct CommissionId(uuid::Uuid);
