//! `me` over the in-memory fakes: the one implementation both drivers call
//! (ZMVP-205 AC2).

use application::user::{MeError, MeProfile, MeQuery, me};
use domain::elements::profile::Profile as DomainProfile;
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

    let query = MeQuery { user_id: user.id };
    let answer = me(
        query,
        &*runtime.users,
        &*runtime.profile_cache,
        &*runtime.profile_source,
    )
    .await
    .unwrap();

    assert_eq!(answer.did, did);
    let expected_profile = MeProfile {
        handle: "me.bsky.social".to_string(),
        display_name: Some("Me".to_string()),
        avatar_url: None,
    };
    assert_eq!(answer.profile, Some(expected_profile));
}

#[tokio::test]
async fn an_unknown_id_is_unknown_user() {
    let did = Did::new("did:plc:app-nobody".to_string());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let id = UserId::new(Uuid::now_v7());

    let query = MeQuery { user_id: id };
    let error = me(
        query,
        &*runtime.users,
        &*runtime.profile_cache,
        &*runtime.profile_source,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, MeError::UnknownUser(unknown) if unknown == id));
}

#[test]
fn a_profile_flattens_every_optional() {
    let did = Did::new("did:plc:app-flat".to_string());
    let profile = DomainProfile::new(did, "flat.bsky.social")
        .with_display_name("Flat")
        .with_avatar_url("https://cdn/avatar.png");

    let flat = MeProfile::from(profile);

    let expected = MeProfile {
        handle: "flat.bsky.social".to_string(),
        display_name: Some("Flat".to_string()),
        avatar_url: Some("https://cdn/avatar.png".to_string()),
    };
    assert_eq!(flat, expected);
}
