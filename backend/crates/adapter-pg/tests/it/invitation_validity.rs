//! The shared invitation-validity suite (`test_support::invitation_validity_contract`), run
//! against PostgreSQL: one fresh database per case. Requires a container runtime socket.

use test_support::invitation_validity_contract::Runtime;

/// A fresh database's runtime, and the database kept alive.
async fn fresh() -> (Runtime, impl Sized) {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    (runtime, pg)
}

test_support::invitation_validity_contract_tests!(fresh);

// The races only row locks decide, over PostgreSQL. A holder transaction locks the Account row
// `FOR UPDATE`, so a use case that passed its checks on the pool waits at its unit's first
// statement; the holder commits a concurrent change, and the use case decides again on what
// it then finds. The store-level cases drive one unit by hand instead.

use std::time::Duration;

use adapter_pg::PgPool;
use chrono::Utc;
use domain::elements::{
    account::AccountId,
    invitation::{Invitation, InvitationState},
    role::Role,
    user::UserId,
};
use test_support::account_deletion_contract::{
    AccountError, accept, founded, offer, recognized, seat, try_revoke, try_transfer,
};
use test_support::invitation_validity_contract::{try_grant, try_leave, try_offer};

/// Waits, for at most ten seconds, until `count` connections to this database wait on a lock.
async fn wait_for_lock_waits(pool: &PgPool, count: i64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(pool)
        .await
        .expect("read pg_stat_activity");
        if waiting >= count {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{count} connections did not wait on a lock within ten seconds"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Opens a transaction holding `account`'s row `FOR UPDATE`: every account unit waits at
/// its first statement until the transaction ends.
async fn hold_account(
    pool: &PgPool,
    account: &AccountId,
) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut holding = pool.begin().await.expect("begin the hold");
    sqlx::query("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE")
        .bind(AsRef::<str>::as_ref(account))
        .fetch_one(&mut *holding)
        .await
        .expect("hold the account row");
    holding
}

/// Sets `user`'s role in `account` to `role` inside `holding`.
async fn set_role(
    holding: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: &AccountId,
    user: &UserId,
    role: &str,
) {
    sqlx::query("UPDATE account_members SET role = $1 WHERE account_id = $2 AND user_id = $3")
        .bind(role)
        .bind(AsRef::<str>::as_ref(account))
        .bind(AsRef::<str>::as_ref(user))
        .execute(&mut **holding)
        .await
        .expect("set the role");
}

/// Removes `user`'s membership of `account` inside `holding`.
async fn remove_member(
    holding: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: &AccountId,
    user: &UserId,
) {
    sqlx::query("DELETE FROM account_members WHERE account_id = $1 AND user_id = $2")
        .bind(AsRef::<str>::as_ref(account))
        .bind(AsRef::<str>::as_ref(user))
        .execute(&mut **holding)
        .await
        .expect("remove the member");
}

/// Commits, inside `holding`, what a transfer from `owner` to `heir` commits: the demotion,
/// then the promotion.
async fn hand_over(
    holding: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: &AccountId,
    owner: &UserId,
    heir: &UserId,
) {
    set_role(holding, account, owner, "admin").await;
    set_role(holding, account, heir, "owner").await;
}

/// `user`'s role in `account`, read on the pool.
async fn role_now(pool: &PgPool, account: &AccountId, user: &UserId) -> Option<String> {
    sqlx::query_scalar("SELECT role FROM account_members WHERE account_id = $1 AND user_id = $2")
        .bind(AsRef::<str>::as_ref(account))
        .bind(AsRef::<str>::as_ref(user))
        .fetch_optional(pool)
        .await
        .expect("read the role")
}

/// Founds an Account owned by `did:plc:<tag>owner` with an Admin and a Member seated.
async fn owner_admin_member(runtime: &Runtime, tag: &str) -> (AccountId, UserId, UserId, UserId) {
    let owner = recognized(runtime, &format!("did:plc:{tag}owner")).await;
    let admin = recognized(runtime, &format!("did:plc:{tag}admin")).await;
    let member = recognized(runtime, &format!("did:plc:{tag}member")).await;
    let account = founded(runtime, &owner, &format!("{tag}.zurfur.app")).await;
    seat(runtime, &account, &owner, &admin).await;
    try_grant(runtime, &account, &owner, &admin, Role::Admin)
        .await
        .expect("seat the Admin");
    seat(runtime, &account, &owner, &member).await;
    (account, owner, admin, member)
}

/// Runs `act` in a task while `holding` keeps the Account row, commits `holding` once the act
/// waits, and answers what the act answered.
async fn after_commit<F>(
    pool: &PgPool,
    holding: sqlx::Transaction<'static, sqlx::Postgres>,
    act: F,
) -> Result<(), AccountError>
where
    F: std::future::Future<Output = Result<(), AccountError>> + Send + 'static,
{
    let waiting = tokio::spawn(act);
    wait_for_lock_waits(pool, 1).await;
    holding
        .commit()
        .await
        .expect("commit the concurrent change");
    waiting.await.expect("the act finishes")
}

// The issue's unit holds its issuer's row shared; the demotion's role update waits for it, and
// its revoke, a separate statement after the update, sees the offer committed meanwhile.
#[tokio::test]
async fn an_offer_committed_while_its_issuer_is_demoted_is_revoked() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, admin, _member) = owner_admin_member(&runtime, "racecommitted").await;
    let invitee = recognized(&runtime, "did:plc:racecommittedinvitee").await;

    let mut issuing = runtime.database.begin().await.expect("begin the issue");
    let issuer = issuing
        .accounts()
        .role_of_for_share(&admin, &account)
        .await
        .expect("lock the issuer");
    assert_eq!(issuer, Some(Role::Admin));
    let invitation = Invitation::issue(
        account.clone(),
        invitee.clone(),
        Role::Member,
        admin.clone(),
        Utc::now(),
    );
    issuing
        .accounts()
        .create_invitation(&invitation)
        .await
        .expect("insert the offer");

    let database = runtime.database.clone();
    let (demoted_account, demoted) = (account.clone(), admin.clone());
    let demotion = tokio::spawn(async move {
        let mut unit = database.begin().await?;
        unit.accounts()
            .grant_role(&demoted, &demoted_account, &Role::Member)
            .await?;
        unit.commit().await
    });
    wait_for_lock_waits(&runtime.pool, 1).await;
    issuing.commit().await.expect("commit the offer");
    demotion
        .await
        .expect("the demotion finishes")
        .expect("the demotion commits");

    let stored = runtime
        .accounts
        .find_invitation(&invitation.id)
        .await
        .expect("read the offer")
        .expect("the offer is stored");
    assert_eq!(stored.state, InvitationState::Revoked);
}

// An issue that passed its check before its issuer's demotion committed decides again in its
// unit, and is refused; no offer is stored.
#[tokio::test]
async fn an_offer_issued_after_its_issuers_demotion_is_refused() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, admin, _member) = owner_admin_member(&runtime, "raceissue").await;
    let invitee = recognized(&runtime, "did:plc:raceissueinvitee").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    set_role(&mut holding, &account, &admin, "member").await;
    let issuer = runtime.clone();
    let (offered_account, offering, offered) = (account.clone(), admin.clone(), invitee.clone());
    let issued = after_commit(&runtime.pool, holding, async move {
        try_offer(&issuer, &offered_account, &offering, &offered, Role::Member).await
    })
    .await;

    assert!(
        matches!(issued, Err(AccountError::IncorrectRole)),
        "the demoted issuer is refused: {issued:?}"
    );
    let pending = runtime
        .accounts
        .find_pending_invitation(&account, &invitee)
        .await
        .expect("read the offers");
    assert!(pending.is_none(), "no offer is stored");
}

// An accept that commits before its issuer's demotion keeps
// its seat at the offered role.
#[tokio::test]
async fn an_accept_committing_before_a_demotion_seats_its_member() {
    let (runtime, _kept) = fresh().await;
    let (account, owner, admin, _member) = owner_admin_member(&runtime, "raceaccepted").await;
    let invitee = recognized(&runtime, "did:plc:raceacceptedinvitee").await;
    offer(&runtime, &account, &admin, &invitee, Role::Manager).await;

    accept(&runtime, &account, &invitee).await;
    try_grant(&runtime, &account, &owner, &admin, Role::Member)
        .await
        .expect("demote the issuer");

    assert_eq!(
        role_now(&runtime.pool, &account, &invitee).await.as_deref(),
        Some("manager")
    );
}

// A leave that passed its check before a transfer made the leaver the Owner answers
// `OwnerCannotLeave`, and the new Owner stays.
#[tokio::test]
async fn a_leave_losing_to_a_transfer_answers_owner_cannot_leave() {
    let (runtime, _kept) = fresh().await;
    let (account, owner, _admin, member) = owner_admin_member(&runtime, "raceleave").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    hand_over(&mut holding, &account, &owner, &member).await;
    let leaver = runtime.clone();
    let (left, leaving) = (account.clone(), member.clone());
    let left_now = after_commit(&runtime.pool, holding, async move {
        try_leave(&leaver, &left, &leaving).await
    })
    .await;

    assert!(
        matches!(left_now, Err(AccountError::OwnerCannotLeave)),
        "the new Owner can't leave: {left_now:?}"
    );
    assert_eq!(
        role_now(&runtime.pool, &account, &member).await.as_deref(),
        Some("owner")
    );
}

// A revoke that passed its check before a transfer made its target the Owner answers
// `IncorrectRole`, and the new Owner stays.
#[tokio::test]
async fn a_revoke_losing_to_a_transfer_answers_incorrect_role() {
    let (runtime, _kept) = fresh().await;
    let (account, owner, admin, member) = owner_admin_member(&runtime, "racerevoke").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    hand_over(&mut holding, &account, &owner, &member).await;
    let revoker = runtime.clone();
    let (revoked, revoking, target) = (account.clone(), admin.clone(), member.clone());
    let revoked_now = after_commit(&runtime.pool, holding, async move {
        try_revoke(&revoker, &revoked, &revoking, &target).await
    })
    .await;

    assert!(
        matches!(revoked_now, Err(AccountError::IncorrectRole)),
        "an Admin can't revoke the new Owner: {revoked_now:?}"
    );
    assert_eq!(
        role_now(&runtime.pool, &account, &member).await.as_deref(),
        Some("owner")
    );
}

// A transfer that passed its check before its heir left answers `NotAMember`, and the Owner
// stays Owner.
#[tokio::test]
async fn a_transfer_whose_heir_left_meanwhile_answers_member_not_found() {
    let (runtime, _kept) = fresh().await;
    let (account, owner, _admin, member) = owner_admin_member(&runtime, "raceheir").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    remove_member(&mut holding, &account, &member).await;
    let transferrer = runtime.clone();
    let (transferred, transferring, heir) = (account.clone(), owner.clone(), member.clone());
    let transferred_now = after_commit(&runtime.pool, holding, async move {
        try_transfer(&transferrer, &transferred, &transferring, &heir).await
    })
    .await;

    assert!(
        matches!(transferred_now, Err(AccountError::NotAMember)),
        "the heir is gone: {transferred_now:?}"
    );
    assert_eq!(
        role_now(&runtime.pool, &account, &owner).await.as_deref(),
        Some("owner")
    );
}

// A grant by an Admin demoted after their check passed is refused, and the target keeps
// their role.
#[tokio::test]
async fn a_grant_by_an_admin_demoted_meanwhile_is_refused() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, admin, member) = owner_admin_member(&runtime, "racegrant").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    set_role(&mut holding, &account, &admin, "member").await;
    let granter = runtime.clone();
    let (granted_in, granting, target) = (account.clone(), admin.clone(), member.clone());
    let granted_now = after_commit(&runtime.pool, holding, async move {
        try_grant(&granter, &granted_in, &granting, &target, Role::Manager).await
    })
    .await;

    assert!(
        matches!(granted_now, Err(AccountError::IncorrectRole)),
        "the demoted Admin is refused: {granted_now:?}"
    );
    assert_eq!(
        role_now(&runtime.pool, &account, &member).await.as_deref(),
        Some("member")
    );
}

// A revoke by an Admin demoted after their check passed is refused, and the target stays.
#[tokio::test]
async fn a_revoke_by_an_admin_demoted_meanwhile_is_refused() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, admin, member) = owner_admin_member(&runtime, "racedemoted").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    set_role(&mut holding, &account, &admin, "member").await;
    let revoker = runtime.clone();
    let (revoked, revoking, target) = (account.clone(), admin.clone(), member.clone());
    let revoked_now = after_commit(&runtime.pool, holding, async move {
        try_revoke(&revoker, &revoked, &revoking, &target).await
    })
    .await;

    assert!(
        matches!(revoked_now, Err(AccountError::IncorrectRole)),
        "the demoted Admin is refused: {revoked_now:?}"
    );
    assert_eq!(
        role_now(&runtime.pool, &account, &member).await.as_deref(),
        Some("member")
    );
}

// The pool check refuses a stranger with the same answer whether or not the target is a
// member, so a revoke can't probe membership.
#[tokio::test]
async fn a_stranger_revoking_gets_forbidden_whether_or_not_the_target_is_a_member() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, _admin, member) = owner_admin_member(&runtime, "racestranger").await;
    let stranger = recognized(&runtime, "did:plc:racestrangerstranger").await;
    let nobody = recognized(&runtime, "did:plc:racestrangernobody").await;

    let of_a_member = try_revoke(&runtime, &account, &stranger, &member).await;
    let of_a_nobody = try_revoke(&runtime, &account, &stranger, &nobody).await;

    assert!(
        matches!(of_a_member, Err(AccountError::IncorrectRole)),
        "{of_a_member:?}"
    );
    assert!(
        matches!(of_a_nobody, Err(AccountError::IncorrectRole)),
        "{of_a_nobody:?}"
    );
}

// Inside the unit too, the actor's standing is decided before the target's membership: an
// Admin demoted meanwhile gets `IncorrectRole` even when the target also left.
#[tokio::test]
async fn an_admin_demoted_meanwhile_gets_forbidden_whether_or_not_the_target_left() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, admin, member) = owner_admin_member(&runtime, "raceoracle").await;

    let mut holding = hold_account(&runtime.pool, &account).await;
    set_role(&mut holding, &account, &admin, "member").await;
    remove_member(&mut holding, &account, &member).await;
    let revoker = runtime.clone();
    let (revoked, revoking, target) = (account.clone(), admin.clone(), member.clone());
    let revoked_now = after_commit(&runtime.pool, holding, async move {
        try_revoke(&revoker, &revoked, &revoking, &target).await
    })
    .await;

    assert!(
        matches!(revoked_now, Err(AccountError::IncorrectRole)),
        "the actor's standing answers first: {revoked_now:?}"
    );
}

/// Waits, for at most ten seconds, until a connection to this database waits on a lock that
/// the backend `holder` holds.
async fn wait_for_a_wait_on(pool: &PgPool, holder: i32) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE datname = current_database() AND $1 = ANY (pg_blocking_pids(pid))",
        )
        .bind(holder)
        .fetch_one(pool)
        .await
        .expect("read the blocked backends");
        if waiting >= 1 {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no connection waited on backend {holder} within ten seconds"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Opens a transaction holding `user`'s membership row in `account` `FOR SHARE`, and answers
/// it with its backend's pid.
async fn hold_member_shared(
    pool: &PgPool,
    account: &AccountId,
    user: &UserId,
) -> (sqlx::Transaction<'static, sqlx::Postgres>, i32) {
    let mut holding = pool.begin().await.expect("begin the member hold");
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *holding)
        .await
        .expect("read the hold's pid");
    sqlx::query("SELECT 1 FROM account_members WHERE account_id = $1 AND user_id = $2 FOR SHARE")
        .bind(AsRef::<str>::as_ref(account))
        .bind(AsRef::<str>::as_ref(user))
        .fetch_one(&mut *holding)
        .await
        .expect("hold the member row");
    (holding, pid)
}

// Two grants on each other's rows, each having passed its check under opposite ranks, lock
// both rows in ascending DID order: X holds the lower row shared and waits on the higher one,
// Y waits on the lower one behind X, so X decides first and Y then finishes. Locking the actor
// first instead lets Y share the higher row while X waits to update it: a deadlock, which
// PostgreSQL breaks by aborting one unit. The store's refusal triggers are off on this
// database: the rank swap must commit while the Account row is held.
#[tokio::test]
async fn two_units_locking_each_others_rows_through_the_helper_both_finish() {
    let (runtime, _kept) = fresh().await;
    let (account, _owner, lower, higher) = owner_admin_member(&runtime, "racecycle").await;
    assert!(
        AsRef::<str>::as_ref(&lower) < AsRef::<str>::as_ref(&higher),
        "X's actor holds the lower DID"
    );
    sqlx::query("ALTER TABLE account_members DISABLE TRIGGER account_members_refuse_deactivated")
        .execute(&runtime.pool)
        .await
        .expect("disable the refusal");

    // X passes its check while `lower` is the Admin, then waits at its unit's first statement.
    let account_hold = hold_account(&runtime.pool, &account).await;
    let x_runtime = runtime.clone();
    let (x_account, x_actor, x_target) = (account.clone(), lower.clone(), higher.clone());
    let x = tokio::spawn(async move {
        try_grant(&x_runtime, &x_account, &x_actor, &x_target, Role::Manager).await
    });
    wait_for_lock_waits(&runtime.pool, 1).await;

    let mut swapping = runtime.pool.begin().await.expect("begin the swap");
    set_role(&mut swapping, &account, &higher, "admin").await;
    set_role(&mut swapping, &account, &lower, "member").await;
    swapping.commit().await.expect("swap the ranks");

    // X takes the lower row shared, then waits to update the higher row behind the share.
    let (member_hold, member_holder) = hold_member_shared(&runtime.pool, &account, &higher).await;
    account_hold.rollback().await.expect("release the account");
    wait_for_a_wait_on(&runtime.pool, member_holder).await;

    // Y passes its check while `higher` is the Admin; it waits on the lower row behind X.
    let y_runtime = runtime.clone();
    let (y_account, y_actor, y_target) = (account.clone(), higher.clone(), lower.clone());
    let y = tokio::spawn(async move {
        try_grant(&y_runtime, &y_account, &y_actor, &y_target, Role::Manager).await
    });
    wait_for_lock_waits(&runtime.pool, 2).await;
    member_hold
        .rollback()
        .await
        .expect("release the member row");

    let x_now = x.await.expect("X finishes");
    let y_now = y.await.expect("Y finishes");
    assert!(
        matches!(x_now, Err(AccountError::IncorrectRole)),
        "X, now a Member, is refused: {x_now:?}"
    );
    assert!(y_now.is_ok(), "Y, now the Admin, grants: {y_now:?}");
    assert_eq!(
        role_now(&runtime.pool, &account, &lower).await.as_deref(),
        Some("manager")
    );
}
