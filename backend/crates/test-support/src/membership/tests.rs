use super::*;
use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountName, ListingScope},
    did::Did,
    handle::Handle,
};

#[tokio::test]
async fn a_seated_member_is_listed_on_their_profile() -> anyhow::Result<()> {
    let backend = MemBackend::new();
    let now = Utc::now();
    let owner = UserId::from(Did::from("did:plc:seatowner".to_string()));
    let member = UserId::from(Did::from("did:plc:seatmember".to_string()));
    let account = Account {
        id: AccountId::from(Did::from("did:plc:seatstudio".to_string())),
        handle: "seatstudio.example.com".parse::<Handle>()?,
        name: "Seat Studio".parse::<AccountName>()?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    let ownership = UserAccount {
        user_id: owner.clone(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };
    let mut founding = backend.database().begin().await?;
    founding.accounts().create(&account, &ownership).await?;
    founding.commit().await?;

    let mut seating = backend.database().begin().await?;
    seat_member(
        &mut *seating,
        &account.id,
        &member,
        Role::Member,
        &owner,
        now,
    )
    .await?;
    seating.commit().await?;

    let public_listing = backend
        .account_store()
        .list_for_user(&member, ListingScope::PublicProfile)
        .await?;
    let listed_ids: Vec<&AccountId> = public_listing
        .iter()
        .map(|membership| &membership.account.id)
        .collect();
    assert_eq!(
        listed_ids,
        vec![&account.id],
        "a member seated by seat_member shows the Account on their public profile"
    );
    Ok(())
}
