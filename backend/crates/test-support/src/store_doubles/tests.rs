use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{commission::CommissionRole, level::Level};

use super::*;

// Scripted apart, the pool's standing and a unit's standing_for_update each answer their own
// script; an unscripted pair reads through to the real store.
#[tokio::test]
async fn the_pool_and_the_unit_are_scripted_apart() -> anyhow::Result<()> {
    let backend = MemBackend::new();
    let owner = UserId::from(Did::from("did:plc:scriptowner".to_string()));
    let stranger = UserId::from(Did::from("did:plc:scriptstranger".to_string()));
    let title = "A scripted commission".parse::<CommissionTitle>()?;
    let commission = Commission::create(title, owner.clone(), Utc::now(), None);
    backend.create_commission(&commission).await?;
    let script = StandingScript::default();
    let store = ScriptedStanding::new(backend.commission_store(), script.clone());
    let database = ScriptedStandingDatabase::new(backend.database(), script.clone());

    let as_owner = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: false,
    };
    script.answer_standing(commission.id, stranger.clone(), Some(as_owner));
    script.answer_standing_for_update(commission.id, stranger.clone(), None);

    let pooled = store.standing(&commission.id, &stranger).await?;
    assert_eq!(pooled, Some(as_owner), "the pool answers its script");
    let mut unit = database.begin().await?;
    let locked = unit
        .commissions()
        .standing_for_update(&commission.id, &stranger)
        .await?;
    assert_eq!(locked, None, "the unit answers its own script");
    let read_through = unit
        .commissions()
        .standing_for_update(&commission.id, &owner)
        .await?;
    assert_eq!(
        read_through,
        Some(as_owner),
        "an unscripted pair reads through"
    );
    unit.rollback().await?;

    let unscripted = store.standing(&commission.id, &owner).await?;
    assert_eq!(unscripted, Some(as_owner), "the owner reads through");
    Ok(())
}
