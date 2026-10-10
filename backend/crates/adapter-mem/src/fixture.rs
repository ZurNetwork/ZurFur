//! The fixture world: fixed rows each team seeds into a [`MemBackend`](crate::MemBackend)
//! for the dev profile and the tests. Each team owns its own leaf.

mod commission;

pub use commission::{
    old_sketch, queue, seed_commission, untitled, untitled_files, untitled_slots,
};
