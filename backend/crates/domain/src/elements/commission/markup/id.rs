use std::ops::Deref;

/// The app-private, opaque key of one stored markup (UUIDv7) — the
/// `commission_markup` row's primary key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkupKey(uuid::Uuid);

impl MarkupKey {
    /// Wrap an already-minted UUIDv7.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh key for a new markup. Sorts as creation order, so a
    /// per-file read needs no separate ordering column.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for MarkupKey {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
