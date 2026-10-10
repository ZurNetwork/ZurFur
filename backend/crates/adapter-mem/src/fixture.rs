//! The fixture world: fixed rows each team seeds into a [`MemBackend`](crate::MemBackend)
//! for the dev profile and the tests. Each team owns its own leaf.

mod identity;

pub use identity::{abco, alice, bob, ember, kael_sona, seed_identity, supreme_arts};
