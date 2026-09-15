use serde::Deserialize;

/// The app-private key of one element of a commission's composition (UUIDv7).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
    derive_more::Into,
)]
#[serde(transparent)]
pub struct ElementId(uuid::Uuid);
pub type SeatId = ElementId;

impl ElementId {
    /// Mint a fresh UUIDv7 element key; also used by the satellite shapes that
    /// ride an element.
    pub(in crate::elements::commission) fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

/// The app-private key of one tab of a commission (UUIDv7). Tabs are the only
/// composition level with a row of their own.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
    derive_more::Into,
)]
pub struct TabId(uuid::Uuid);

impl TabId {
    /// Mint a fresh UUIDv7 tab key, as done when a commission's skeleton tabs
    /// are created alongside it.
    pub fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}
