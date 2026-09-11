//! `me` over the in-memory fakes: the one implementation both drivers call
//! (ZMVP-205 AC2).

use application::user::{MeError, me};
use domain::elements::{did::Did, profile::Profile, user::UserId};
use domain::ports::UnitOfWork;
use uuid::Uuid;

#[tokio::test]
async fn a_recognized_user_gets_their_profile() {
    let did = Did::new("did:plc:app-me".to_string());
    let profile = Profile::new(did.clone(), "me.bsky.social").with_display_name("Me");
    let fixture = test_support::runtime::mem(&did)
        .profile(profile.clone())
        .build();
    let runtime = fixture.runtime;
    let provisioned = did.clone();
    let user = runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&provisioned).await
        })
        .await
        .unwrap();

    let answer = me(
        &*runtime.users,
        &*runtime.profile_cache,
        &*runtime.profile_source,
        user.id,
    )
    .await
    .unwrap();

    assert_eq!(answer.user.did, did);
    assert_eq!(answer.profile, Some(profile));
}

#[tokio::test]
async fn an_unknown_id_is_unknown_user() {
    let did = Did::new("did:plc:app-nobody".to_string());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let id = UserId::new(Uuid::now_v7());

    let error = me(
        &*runtime.users,
        &*runtime.profile_cache,
        &*runtime.profile_source,
        id,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, MeError::UnknownUser(unknown) if unknown == id));
}
