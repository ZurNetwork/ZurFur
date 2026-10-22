//! The shared node conformance suite (`test_support::node_contract`), run against
//! the in-memory node map: one fresh backend per case.

use std::sync::Arc;

use adapter_mem::MemBackend;
use domain::ports::{Database, NodeStore};

/// A fresh backend's database and node store.
async fn fresh() -> (Arc<dyn Database>, Arc<dyn NodeStore>, ()) {
    let backend = MemBackend::new();
    (backend.database(), backend.node_store(), ())
}

test_support::node_contract_tests!(fresh);
