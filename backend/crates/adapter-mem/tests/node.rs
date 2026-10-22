//! The shared node conformance suite (`test_support::node_contract`), run against
//! the in-memory node map: one fresh backend per case, each committed commission node bound
//! to its commission through the minting seeder.

mod common;

use std::sync::Arc;

use adapter_mem::MemBackend;
use domain::ports::{Database, NodeStore};
use test_support::seeder::CommissionSeeder;

/// A fresh backend's database, node store and commission seeder.
async fn fresh() -> (
    Arc<dyn Database>,
    Arc<dyn NodeStore>,
    Arc<dyn CommissionSeeder>,
    (),
) {
    let backend = MemBackend::new();
    let seeder: Arc<dyn CommissionSeeder> = Arc::new(common::MintingSeeder);
    (backend.database(), backend.node_store(), seeder, ())
}

test_support::node_contract_tests!(fresh);
