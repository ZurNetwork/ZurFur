//! The shared commission-standing suite (`test_support::commission_standing_contract`), run
//! against the in-memory commission maps: one fresh backend per case.

use std::sync::Arc;

use adapter_mem::MemBackend;
use async_trait::async_trait;
use domain::elements::{commission::CommissionId, user::UserId};
use domain::ports::{CommissionStore, Database};
use test_support::commission_standing_contract::ParticipantRows;

/// The backend's participant map, written directly: no port writes a non-owner row yet.
struct MemParticipantRows(MemBackend);

#[async_trait]
impl ParticipantRows for MemParticipantRows {
    async fn add(&self, commission: &CommissionId, user: &UserId) {
        self.0
            .seed_participant(*commission, user.clone(), chrono::Utc::now());
    }

    async fn remove(&self, commission: &CommissionId, user: &UserId) {
        self.0.remove_participant(*commission, user);
    }
}

/// A fresh backend's database, commission store and participant rows.
async fn fresh() -> (
    Arc<dyn Database>,
    Arc<dyn CommissionStore>,
    Arc<dyn ParticipantRows>,
    (),
) {
    let backend = MemBackend::new();
    let rows = MemParticipantRows(backend.clone());
    (
        backend.database(),
        backend.commission_store(),
        Arc::new(rows),
        (),
    )
}

test_support::commission_standing_contract_tests!(fresh);
