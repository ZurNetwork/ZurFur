//! The shared invitation-validity conformance suite, run by both adapters: `adapter-mem`'s
//! `tests/invitation_validity.rs` and `adapter-pg`'s `tests/it/invitation_validity.rs`.
//!
//! Each case takes a fresh [`Runtime`] over one store and drives the account use cases as a
//! driver would, then reads the offers back from the store. It checks that a role change
//! revokes the pending offers its member can no longer make, and nothing else: a demotion
//! to a role that offers nothing, a transfer that makes the old Owner an Admin, and the
//! changes that must leave every offer alone.
//! [`invitation_validity_contract_tests!`](crate::invitation_validity_contract_tests)
//! writes one test per case.

use chrono::Utc;
use domain::elements::{
    account::AccountId,
    invitation::{Invitation, InvitationState},
    role::Role,
    user::UserId,
};
use domain::ports::UnitOfWork;

use application::account::{self, invitation};

pub use composition::Runtime;

use crate::account_deletion_contract::{
    AccountError, accept, founded, offer, recognized, seat, try_transfer,
};

/// Writes one `#[tokio::test]` per suite case. `$setup` is an async fn returning
/// `(Runtime, impl Sized)` over one fresh store; the second item is kept alive for the test.
#[macro_export]
macro_rules! invitation_validity_contract_tests {
    ($setup:path) => {
        $crate::invitation_validity_contract_tests!(@cases $setup;
            demoting_an_admin_to_manager_revokes_all_their_pending_offers,
            demoting_an_admin_to_member_revokes_all_their_pending_offers,
            a_transfer_revokes_the_old_owners_admin_offers_and_keeps_manager_and_member_offers,
            a_promotion_revokes_nothing,
            regranting_the_same_role_revokes_nothing,
            the_revoke_touches_only_this_issuers_pending_offers_here,
        );
    };
    (@cases $setup:path; $($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                let (runtime, _kept) = $setup().await;
                $crate::invitation_validity_contract::$case(&runtime).await;
            }
        )*
    };
}

/// `actor` grants `target` `role` in `account`, answering what the use case answered.
pub async fn try_grant(
    runtime: &Runtime,
    account: &AccountId,
    actor: &UserId,
    target: &UserId,
    role: Role,
) -> Result<(), AccountError> {
    let granting = account::role::grant::Command {
        account_id: account.clone(),
        target_id: target.clone(),
        actor_id: actor.clone(),
        role,
    };
    let app = runtime.app();
    app.accounts().roles().grant(granting).await.map(|_| ())
}

/// `user` leaves `account`, answering what the use case answered.
pub async fn try_leave(
    runtime: &Runtime,
    account: &AccountId,
    user: &UserId,
) -> Result<(), AccountError> {
    let leaving = account::leave::Command {
        leaving_user_id: user.clone(),
        account_id: account.clone(),
    };
    runtime.app().accounts().leave(leaving).await.map(|_| ())
}

/// `inviter` offers `invitee` `role` in `account`, answering what the use case answered.
pub async fn try_offer(
    runtime: &Runtime,
    account: &AccountId,
    inviter: &UserId,
    invitee: &UserId,
    role: Role,
) -> Result<(), AccountError> {
    let issuing = account::invitation::issue::Command {
        account_id: account.clone(),
        target_id: invitee.clone(),
        actor_id: inviter.clone(),
        role,
    };
    let app = runtime.app();
    app.accounts()
        .invitations()
        .issue(issuing, Utc::now())
        .await
        .map(|_| ())
}

/// `actor` changes `target`'s role in `account` to `role`, which must succeed.
async fn granted(
    runtime: &Runtime,
    account: &AccountId,
    actor: &UserId,
    target: &UserId,
    role: Role,
) {
    let granting = account::role::grant::Command {
        account_id: account.clone(),
        target_id: target.clone(),
        actor_id: actor.clone(),
        role,
    };
    let app = runtime.app();
    app.accounts()
        .roles()
        .grant(granting)
        .await
        .expect("grant the role");
}

/// Seats `member` in `account` as an Admin: `owner` offers Member, `member` accepts, `owner`
/// grants Admin.
async fn seat_admin(runtime: &Runtime, account: &AccountId, owner: &UserId, member: &UserId) {
    seat(runtime, account, owner, member).await;
    granted(runtime, account, owner, member, Role::Admin).await;
}

/// `inviter` offers `invitee` a seat at `role`, and answers the stored pending offer.
async fn offered(
    runtime: &Runtime,
    account: &AccountId,
    inviter: &UserId,
    invitee: &UserId,
    role: Role,
) -> Invitation {
    offer(runtime, account, inviter, invitee, role).await;
    runtime
        .accounts
        .find_pending_invitation(account, invitee)
        .await
        .expect("read the offer")
        .expect("the offer is pending")
}

/// The stored invitation `offer` names, in whatever state it holds now.
async fn reread(runtime: &Runtime, offer: &Invitation) -> Invitation {
    runtime
        .accounts
        .find_invitation(&offer.id)
        .await
        .expect("read the invitation")
        .expect("the invitation is stored")
}

/// Asserts `offer`, read earlier, is stored in `state` now. An offer that was pending and is
/// revoked has a later `updated_at`; any other keeps the one it was read with.
async fn assert_state(runtime: &Runtime, offer: &Invitation, state: InvitationState, label: &str) {
    let stored = reread(runtime, offer).await;
    assert_eq!(stored.state, state, "{label}: state");
    match state {
        InvitationState::Revoked if offer.state == InvitationState::Pending => {
            assert!(stored.updated_at >= offer.updated_at, "{label}: updated_at");
        }
        _ => assert_eq!(
            stored.updated_at, offer.updated_at,
            "{label}: updated_at kept"
        ),
    }
}

/// Demotes an Admin's two pending offers away: every one the Admin issued is revoked, and
/// the Owner's own offer stays.
async fn demotion_revokes_every_offer_of_the_demoted(runtime: &Runtime, to: Role, tag: &str) {
    let owner = recognized(runtime, &format!("did:plc:{tag}owner")).await;
    let admin = recognized(runtime, &format!("did:plc:{tag}admin")).await;
    let first_invitee = recognized(runtime, &format!("did:plc:{tag}first")).await;
    let second_invitee = recognized(runtime, &format!("did:plc:{tag}second")).await;
    let owners_invitee = recognized(runtime, &format!("did:plc:{tag}third")).await;
    let account = founded(runtime, &owner, &format!("{tag}.zurfur.app")).await;
    seat_admin(runtime, &account, &owner, &admin).await;
    let manager_offer = offered(runtime, &account, &admin, &first_invitee, Role::Manager).await;
    let member_offer = offered(runtime, &account, &admin, &second_invitee, Role::Member).await;
    let owners_offer = offered(runtime, &account, &owner, &owners_invitee, Role::Admin).await;

    granted(runtime, &account, &owner, &admin, to).await;

    assert_state(
        runtime,
        &manager_offer,
        InvitationState::Revoked,
        "manager offer",
    )
    .await;
    assert_state(
        runtime,
        &member_offer,
        InvitationState::Revoked,
        "member offer",
    )
    .await;
    assert_state(
        runtime,
        &owners_offer,
        InvitationState::Pending,
        "the Owner's offer",
    )
    .await;
}

// R-240: an Admin demoted to Manager can offer nothing, so every pending offer they issued
// is revoked with the demotion.
pub async fn demoting_an_admin_to_manager_revokes_all_their_pending_offers(runtime: &Runtime) {
    demotion_revokes_every_offer_of_the_demoted(runtime, Role::Manager, "tomanager").await;
}

// The same for a demotion to Member.
pub async fn demoting_an_admin_to_member_revokes_all_their_pending_offers(runtime: &Runtime) {
    demotion_revokes_every_offer_of_the_demoted(runtime, Role::Member, "tomember").await;
}

// R-230: a transfer makes the old Owner an Admin, who can't offer Admin: their pending Admin
// offer is revoked, their Manager and Member offers stay.
pub async fn a_transfer_revokes_the_old_owners_admin_offers_and_keeps_manager_and_member_offers(
    runtime: &Runtime,
) {
    let owner = recognized(runtime, "did:plc:handoverowner").await;
    let heir = recognized(runtime, "did:plc:handoverheir").await;
    let admin_invitee = recognized(runtime, "did:plc:handoveradmin").await;
    let manager_invitee = recognized(runtime, "did:plc:handovermanager").await;
    let member_invitee = recognized(runtime, "did:plc:handovermember").await;
    let account = founded(runtime, &owner, "handover.zurfur.app").await;
    seat(runtime, &account, &owner, &heir).await;
    let admin_offer = offered(runtime, &account, &owner, &admin_invitee, Role::Admin).await;
    let manager_offer = offered(runtime, &account, &owner, &manager_invitee, Role::Manager).await;
    let member_offer = offered(runtime, &account, &owner, &member_invitee, Role::Member).await;

    try_transfer(runtime, &account, &owner, &heir)
        .await
        .expect("transfer the Account");

    assert_state(
        runtime,
        &admin_offer,
        InvitationState::Revoked,
        "admin offer",
    )
    .await;
    assert_state(
        runtime,
        &manager_offer,
        InvitationState::Pending,
        "manager offer",
    )
    .await;
    assert_state(
        runtime,
        &member_offer,
        InvitationState::Pending,
        "member offer",
    )
    .await;
}

// A promotion widens what a member may offer, so it revokes nothing: a Manager's pending
// offer (planted through the store, since no use case lets a Manager issue one) survives
// the Manager's promotion to Admin.
pub async fn a_promotion_revokes_nothing(runtime: &Runtime) {
    let owner = recognized(runtime, "did:plc:promoteowner").await;
    let manager = recognized(runtime, "did:plc:promotemanager").await;
    let invitee = recognized(runtime, "did:plc:promoteinvitee").await;
    let account = founded(runtime, &owner, "promote.zurfur.app").await;
    seat(runtime, &account, &owner, &manager).await;
    granted(runtime, &account, &owner, &manager, Role::Manager).await;
    let planted = Invitation::issue(
        account.clone(),
        invitee.clone(),
        Role::Member,
        manager.clone(),
        Utc::now(),
    );
    let stored =
        application::transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
            uow.accounts().create_invitation(&planted).await
        })
        .await
        .expect("plant the offer");

    granted(runtime, &account, &owner, &manager, Role::Admin).await;

    assert_state(
        runtime,
        &stored,
        InvitationState::Pending,
        "the promoted member's offer",
    )
    .await;
}

// Granting the role a member already holds changes nothing, so an Admin's pending offers
// survive the Owner granting Admin to them again.
pub async fn regranting_the_same_role_revokes_nothing(runtime: &Runtime) {
    let owner = recognized(runtime, "did:plc:regrantowner").await;
    let admin = recognized(runtime, "did:plc:regrantadmin").await;
    let first_invitee = recognized(runtime, "did:plc:regrantfirst").await;
    let second_invitee = recognized(runtime, "did:plc:regrantsecond").await;
    let account = founded(runtime, &owner, "regrant.zurfur.app").await;
    seat_admin(runtime, &account, &owner, &admin).await;
    let manager_offer = offered(runtime, &account, &admin, &first_invitee, Role::Manager).await;
    let member_offer = offered(runtime, &account, &admin, &second_invitee, Role::Member).await;

    granted(runtime, &account, &owner, &admin, Role::Admin).await;

    assert_state(
        runtime,
        &manager_offer,
        InvitationState::Pending,
        "manager offer",
    )
    .await;
    assert_state(
        runtime,
        &member_offer,
        InvitationState::Pending,
        "member offer",
    )
    .await;
}

// The revoke changes only the demoted issuer's pending offers in the Account whose role
// changed: another issuer's offer, the same issuer's offer in another Account, an accepted
// row and an already revoked row keep their state and `updated_at`.
pub async fn the_revoke_touches_only_this_issuers_pending_offers_here(runtime: &Runtime) {
    let owner = recognized(runtime, "did:plc:scopeowner").await;
    let issuer = recognized(runtime, "did:plc:scopeissuer").await;
    let other_issuer = recognized(runtime, "did:plc:scopeother").await;
    let pending_invitee = recognized(runtime, "did:plc:scopepending").await;
    let other_issuers_invitee = recognized(runtime, "did:plc:scopeotherinvitee").await;
    let accepted_invitee = recognized(runtime, "did:plc:scopeaccepted").await;
    let withdrawn_invitee = recognized(runtime, "did:plc:scopewithdrawn").await;
    let elsewhere_invitee = recognized(runtime, "did:plc:scopeelsewhere").await;
    let account = founded(runtime, &owner, "scope.zurfur.app").await;
    let other_account = founded(runtime, &owner, "scopeelsewhere.zurfur.app").await;
    seat_admin(runtime, &account, &owner, &issuer).await;
    seat_admin(runtime, &account, &owner, &other_issuer).await;
    seat_admin(runtime, &other_account, &owner, &issuer).await;
    let pending = offered(runtime, &account, &issuer, &pending_invitee, Role::Manager).await;
    let others = offered(
        runtime,
        &account,
        &other_issuer,
        &other_issuers_invitee,
        Role::Manager,
    )
    .await;
    let elsewhere = offered(
        runtime,
        &other_account,
        &issuer,
        &elsewhere_invitee,
        Role::Manager,
    )
    .await;
    let accepted = offered(runtime, &account, &issuer, &accepted_invitee, Role::Member).await;
    accept(runtime, &account, &accepted_invitee).await;
    let withdrawn = offered(runtime, &account, &issuer, &withdrawn_invitee, Role::Member).await;
    let withdrawing = invitation::revoke::Command {
        account_id: account.clone(),
        actor_id: issuer.clone(),
        target_id: withdrawn_invitee.clone(),
    };
    let app = runtime.app();
    app.accounts()
        .invitations()
        .revoke(withdrawing)
        .await
        .expect("withdraw the offer");
    let withdrawn = reread(runtime, &withdrawn).await;
    let accepted = reread(runtime, &accepted).await;

    granted(runtime, &account, &owner, &issuer, Role::Manager).await;

    assert_state(
        runtime,
        &pending,
        InvitationState::Revoked,
        "the demoted issuer's offer",
    )
    .await;
    assert_state(
        runtime,
        &others,
        InvitationState::Pending,
        "another issuer's offer",
    )
    .await;
    assert_state(
        runtime,
        &elsewhere,
        InvitationState::Pending,
        "the offer in another Account",
    )
    .await;
    assert_state(
        runtime,
        &accepted,
        InvitationState::Accepted,
        "the accepted offer",
    )
    .await;
    assert_state(
        runtime,
        &withdrawn,
        InvitationState::Revoked,
        "the withdrawn offer",
    )
    .await;
}
