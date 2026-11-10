use adapter_mem::{CommissionSeed, MemBackend};
use domain::elements::{
    commission::{CommissionRole, CommissionTitle},
    level::Level,
};

use super::*;

// Scripted apart, the pool's standing and a unit's standing_for_update each answer their own
// script; an unscripted pair reads through to the real store.
#[tokio::test]
async fn the_pool_and_the_unit_are_scripted_apart() -> anyhow::Result<()> {
    let backend = MemBackend::new();
    let owner = backend
        .provision(&Did::from("did:plc:scriptowner".to_string()))
        .await?
        .id;
    let stranger = UserId::from(Did::from("did:plc:scriptstranger".to_string()));
    let title = "A scripted commission".parse::<CommissionTitle>()?;
    let commission = CommissionSeed::private(title, owner.clone());
    let node = backend.seed_bound_commission(&commission).await?;
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
    let owner_locked = LockedStanding {
        standing: as_owner,
        node,
    };
    assert_eq!(
        read_through,
        Some(owner_locked),
        "an unscripted pair reads through"
    );
    unit.rollback().await?;

    let unscripted = store.standing(&commission.id, &owner).await?;
    assert_eq!(unscripted, Some(as_owner), "the owner reads through");
    Ok(())
}
