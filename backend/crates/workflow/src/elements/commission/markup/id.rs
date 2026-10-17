/// The app-private, opaque key of one stored markup (UUIDv7) — the
/// `commission_markup` row's primary key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct MarkupKey(uuid::Uuid);

impl MarkupKey {
    /// Mint a fresh key for a new markup. Sorts as creation order, so a
    /// per-file read needs no separate ordering column.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}
