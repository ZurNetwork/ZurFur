use adapter_mem::MemBackend;

use super::*;

// The recording commission store logs `standing` like its other reads.
#[tokio::test]
async fn standing_is_recorded() -> anyhow::Result<()> {
    let backend = MemBackend::new();
    let log = CallLog::default();
    let store = RecordingCommissionStore::new(backend.commission_store(), log.clone());
    let commission = CommissionId::from(uuid::Uuid::now_v7());
    let viewer = UserId::from(Did::from("did:plc:recordedviewer".to_string()));

    store.standing(&commission, &viewer).await?;
    assert_eq!(log.events(), ["standing"]);
    Ok(())
}
