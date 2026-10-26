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

// The recording whole-row reads log `find` and `list_owned_by` by name.
#[tokio::test]
async fn whole_row_reads_are_recorded() -> anyhow::Result<()> {
    let backend = MemBackend::new();
    let log = CallLog::default();
    let rows = RecordingCommissionRows::new(backend.commission_rows(), log.clone());
    let commission = CommissionId::from(uuid::Uuid::now_v7());
    let owner = UserId::from(Did::from("did:plc:recordedowner".to_string()));

    rows.find(&commission).await?;
    rows.list_owned_by(&owner).await?;
    assert_eq!(log.events(), ["find", "list_owned_by"]);
    Ok(())
}
