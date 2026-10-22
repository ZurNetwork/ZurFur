/// A node's app-private key (UUIDv7). It never leaves the server.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Into,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
)]
pub struct NodeId(uuid::Uuid);

impl NodeId {
    /// Mint a fresh UUIDv7 node key.
    pub fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

#[cfg(test)]
mod tests;
