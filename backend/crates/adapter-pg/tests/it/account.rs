//! Round-trips the account/membership/invitation store against a throwaway
//! PostgreSQL container, proving the migration-created `accounts` /
//! `account_members` / `account_invitations` tables persist a founded account and
//! its Owner membership in one transaction, that reads come back, and that the
//! Unit-of-Work seam commits across aggregates atomically (and rolls back a
//! dropped unit). Reads go through [`PgAccountStore`]; every write goes through the
//! [`PgDatabase`] factory's [`UnitOfWork`] (DD `24150017`). The founder is
//! provisioned first because `account_members.user_id` references `users(id)`.
//! Requires a container runtime socket (DOCKER_HOST honored).
use std::collections::BTreeSet;

use adapter_pg::{
    ACCOUNT_FACT_TABLES, ACCOUNT_NON_FACT_TABLES, PgAccountStore, PgCommissionStore, PgDatabase,
    PgPool,
};
use chrono::{Duration, Utc};
use domain::{
    elements::{
        account::{Account, AccountId, AccountMembership, AccountName, ListingScope},
        commission::{Commission, CommissionTitle, GrantLevel},
        did::Did,
        handle::Handle,
        invitation::{Invitation, InvitationId, InvitationState},
        role::Role,
        user::{User, UserId},
        user_account::UserAccount,
    },
    ports::{AccountStore, CommissionStore, Database, HandleTaken},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

// --- Test helpers: each write opens its own unit of work (begin → op → commit),
// the way a handler does; reads go through the pool-backed read store. They keep
// the test bodies focused on the behavior under test, not the UoW ceremony. ---

/// Recognize a visitor in its own committed unit of work (FK target for memberships).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Found an account with its Owner membership in one unit of work.
async fn create(pool: &PgPool, account: &Account, owner: &UserAccount) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts().create(account, owner).await.expect("create");
    uow.commit().await.expect("commit");
}

/// Soft-delete an account in one unit of work (the real [`AccountWrites::soft_delete`]
/// write path — no more raw-SQL tombstoning).
async fn soft_delete(pool: &PgPool, account: AccountId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .soft_delete(account)
        .await
        .expect("soft_delete");
    uow.commit().await.expect("commit");
}

/// Hard-delete an account in one unit of work (the real
/// [`AccountWrites::hard_delete`] write path).
async fn hard_delete(pool: &PgPool, account: AccountId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .hard_delete(account)
        .await
        .expect("hard_delete");
    uow.commit().await.expect("commit");
}

/// Found an account, returning the `create` result instead of asserting success —
/// so a test can assert the error on a handle collision. Commits only on success
/// (a failed unit rolls back on drop, as in production).
async fn try_create(pool: &PgPool, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let result = uow.accounts().create(account, owner).await;
    if result.is_ok() {
        uow.commit().await.expect("commit");
    }
    result
}

/// Resolve a handle to its live account's `did` off the pool-backed store.
async fn find_did_by_handle(pool: &PgPool, handle: &Handle) -> Option<Did> {
    PgAccountStore::new(pool.clone())
        .find_did_by_handle(handle)
        .await
        .expect("find_did_by_handle")
}

/// Persist a pending invitation in one unit of work.
async fn create_invitation(pool: &PgPool, invitation: &Invitation) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .create_invitation(invitation)
        .await
        .expect("create_invitation");
    uow.commit().await.expect("commit");
}

/// Revoke an invitation in one unit of work.
async fn revoke_invitation(pool: &PgPool, id: InvitationId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .revoke_invitation(id)
        .await
        .expect("revoke_invitation");
    uow.commit().await.expect("commit");
}

/// Accept an invitation (flip + seat) in one unit of work, returning the new membership.
async fn accept_invitation(pool: &PgPool, invitation: Invitation, listed: bool) -> UserAccount {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let member = uow
        .accounts()
        .accept_invitation(invitation, listed)
        .await
        .expect("accept_invitation");
    uow.commit().await.expect("commit");
    member
}

/// A member leaves the account in one unit of work.
async fn leave(pool: &PgPool, user: UserId, account: AccountId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts().leave(user, account).await.expect("leave");
    uow.commit().await.expect("commit");
}

/// An Owner/Admin revokes a member's role in one unit of work.
async fn revoke_role(pool: &PgPool, user: UserId, account: AccountId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .revoke_role(user, account)
        .await
        .expect("revoke_role");
    uow.commit().await.expect("commit");
}

/// Grant (seat or re-seat) a member's role directly, bypassing any invitation.
async fn grant_role(pool: &PgPool, member: &UserAccount) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts().grant_role(member).await.expect("grant_role");
    uow.commit().await.expect("commit");
}

/// Read an account by id off the pool-backed store.
async fn find_account(pool: &PgPool, id: AccountId) -> Option<Account> {
    PgAccountStore::new(pool.clone())
        .find(id)
        .await
        .expect("find")
}

/// Read a member's role off the pool-backed store.
async fn role_of(pool: &PgPool, user: UserId, account: AccountId) -> Option<Role> {
    PgAccountStore::new(pool.clone())
        .role_of(user, account)
        .await
        .expect("role_of")
}

/// Read the lone pending offer for `(account, invited)` off the pool-backed store.
async fn find_pending(pool: &PgPool, account: AccountId, invited: UserId) -> Option<Invitation> {
    PgAccountStore::new(pool.clone())
        .find_pending_invitation(account, invited)
        .await
        .expect("find_pending_invitation")
}

/// Read an invitation by id off the pool-backed store.
async fn find_invitation(pool: &PgPool, id: InvitationId) -> Option<Invitation> {
    PgAccountStore::new(pool.clone())
        .find_invitation(id)
        .await
        .expect("find_invitation")
}

#[tokio::test]
async fn create_persists_the_account_and_its_owner_membership() {
    let (pool, _container) = fresh_pool().await;

    // The founder must exist: account_members.user_id references users(id).
    let owner = provision(&pool, "did:plc:pgowner").await;

    let account_did = Did::new("did:plc:pgacct".to_string());
    let account_handle = "pgacct.example.com".parse::<Handle>().unwrap();
    let account_name = "PG Studio".parse::<AccountName>().unwrap();
    let (account, membership) = Account::open(
        owner.id,
        account_did.clone(),
        account_handle.clone(),
        account_name.clone(),
        Utc::now(),
    );
    let account_id = account.id;
    create(&pool, &account, &membership).await;

    let found = find_account(&pool, account_id)
        .await
        .expect("the founded account is present");
    assert_eq!(found.id, account_id);
    assert_eq!(
        found.did, account_did,
        "the account's minted did round-trips"
    );
    assert_eq!(
        found.handle, account_handle,
        "the account's handle round-trips"
    );
    assert_eq!(found.name, account_name, "the account's name round-trips");
    assert_eq!(found.deleted_at, None, "a freshly founded account is live");

    let role = role_of(&pool, owner.id, account_id).await;
    assert_eq!(
        role,
        Some(Role::Owner(None)),
        "the creating User is the account's Owner"
    );
}

// The Unit of Work commits writes across more than one accessor call atomically:
// founding the account AND issuing an invitation in the SAME `begin()`/`commit()`
// both land. This is the multi-write capability the seam exists for (DD `24150017`).
#[tokio::test]
async fn one_unit_of_work_commits_writes_across_aggregates_atomically() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:multi-o").await;
    let invitee = provision(&pool, "did:plc:multi-invitee").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:multi-acct".to_string()),
        "multi-acct.example.com".parse::<Handle>().unwrap(),
        "Multi Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let invitation = Invitation::issue(
        account.id,
        invitee.id,
        Role::Member(None),
        owner.id,
        Utc::now(),
    );

    // One unit of work, two writes through two accessor calls — then one commit.
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .create(&account, &membership)
        .await
        .expect("found the account on the open tx");
    uow.accounts()
        .create_invitation(&invitation)
        .await
        .expect("issue the invitation on the same tx");
    uow.commit().await.expect("commit lands both writes");

    assert!(
        find_account(&pool, account.id).await.is_some(),
        "the account committed"
    );
    assert!(
        find_pending(&pool, account.id, invitee.id).await.is_some(),
        "the invitation committed in the same unit of work"
    );
}

// Dropping a unit of work before `commit()` rolls back EVERY write in it — the
// `create` wrote two rows (account + membership), and neither survives. This is the
// structural guarantee made observable: an uncommitted unit leaves nothing behind.
#[tokio::test]
async fn a_dropped_unit_of_work_rolls_back_every_write() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:rollback-o").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rollback-acct".to_string()),
        "rollback-acct.example.com".parse::<Handle>().unwrap(),
        "Rollback".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let account_id = account.id;

    // Open the unit, issue the (two-row) write, then drop without committing.
    {
        let db = PgDatabase::new(pool.clone());
        let mut uow = db.begin().await.expect("begin");
        uow.accounts()
            .create(&account, &membership)
            .await
            .expect("create on the open tx");
        // `uow` drops here without `commit` → the transaction rolls back.
    }

    assert!(
        find_account(&pool, account_id).await.is_none(),
        "a dropped unit of work persists no account row"
    );
    assert_eq!(
        role_of(&pool, owner.id, account_id).await,
        None,
        "...and no membership row either — both writes rolled back together"
    );
}

#[tokio::test]
async fn find_unknown_account_is_none() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:pgowner2").await;
    // Founded in the domain but never persisted, so its id is genuinely unknown.
    let (unfounded, _) = Account::open(
        owner.id,
        Did::new("did:plc:ghost".to_string()),
        "ghost.example.com".parse::<Handle>().unwrap(),
        "Ghost".parse::<AccountName>().unwrap(),
        Utc::now(),
    );

    let found = find_account(&pool, unfounded.id).await;
    assert!(
        found.is_none(),
        "an account we never founded resolves to nothing"
    );
}

#[tokio::test]
async fn role_of_non_member_is_none() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:pgowner3").await;
    let stranger = provision(&pool, "did:plc:pgstranger").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:pgacct3".to_string()),
        "pgacct3.example.com".parse::<Handle>().unwrap(),
        "PG Studio 3".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;

    let role = role_of(&pool, stranger.id, account.id).await;
    assert_eq!(role, None, "a user who is not a member holds no role");
}

// ── ZMVP-32 invitations ───────────────────────────────────────────────────────
// Round-trips the invitation methods against the migration-created
// `account_invitations` table (and its partial unique index), mirroring the
// adapter-mem contract suite. The invited user, inviter, and account are
// provisioned first because the table's foreign keys reference users(id)/accounts(id).

/// Provisions an owner and an invitee, founds an account, and returns the handles
/// the invitation tests share. The owner doubles as the inviter.
async fn invitation_fixture(pool: &PgPool, tag: &str) -> (Account, UserId, UserId) {
    let owner = provision(pool, &format!("did:plc:pginviter-{tag}")).await;
    let invitee = provision(pool, &format!("did:plc:pginvitee-{tag}")).await;
    let (account, membership) = Account::open(
        owner.id,
        Did::new(format!("did:plc:pgacct-{tag}")),
        format!("pgacct-{tag}.example.com")
            .parse::<Handle>()
            .unwrap(),
        "PG Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(pool, &account, &membership).await;

    (account, owner.id, invitee.id)
}

// AC3 — a freshly issued pending invitation round-trips: it's the pending offer
// found for its (account, invited_user) pair, with every fact intact.
#[tokio::test]
async fn create_then_find_pending_returns_the_invitation() {
    let (pool, _container) = fresh_pool().await;
    let (account, inviter, invitee) = invitation_fixture(&pool, "rt").await;

    let invitation = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    let id = invitation.id;
    create_invitation(&pool, &invitation).await;

    let found = find_pending(&pool, account.id, invitee)
        .await
        .expect("the pending invitation is found");
    assert_eq!(found.id, id);
    assert_eq!(found.account, account.id);
    assert_eq!(found.invited_user, invitee);
    assert_eq!(
        found.role,
        Role::Admin(None),
        "the offered role round-trips"
    );
    assert_eq!(found.inviter, inviter, "the inviter round-trips (Roles 4a)");
    assert_eq!(found.state, InvitationState::Pending);
}

// AC5 — the partial unique index holds the invariant: a second pending invitation
// for the same pair is absorbed (ON CONFLICT DO NOTHING), not a second row.
#[tokio::test]
async fn a_second_pending_invitation_for_the_same_pair_is_not_a_second_row() {
    let (pool, _container) = fresh_pool().await;
    let (account, inviter, invitee) = invitation_fixture(&pool, "dup").await;

    let first = Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());
    let second = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    create_invitation(&pool, &first).await;
    create_invitation(&pool, &second).await; // a no-op, not an error

    let found = find_pending(&pool, account.id, invitee)
        .await
        .expect("a pending invitation remains");
    assert_eq!(
        found.id, first.id,
        "the first pending offer is the one kept"
    );
    assert!(
        find_invitation(&pool, second.id).await.is_none(),
        "the duplicate issue stored nothing"
    );
}

// AC4 — revoking flips the offer to revoked (guarded UPDATE): it reads back revoked,
// is no longer the live pending offer, and the partial index lets a re-invite through.
#[tokio::test]
async fn revoke_invitation_flips_state_and_clears_the_pending_offer() {
    let (pool, _container) = fresh_pool().await;
    let (account, inviter, invitee) = invitation_fixture(&pool, "rev").await;

    let invitation =
        Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());
    let id = invitation.id;
    create_invitation(&pool, &invitation).await;

    revoke_invitation(&pool, id).await;

    assert_eq!(
        find_invitation(&pool, id).await.map(|i| i.state),
        Some(InvitationState::Revoked),
        "the invitation reads back revoked"
    );
    assert!(
        find_pending(&pool, account.id, invitee).await.is_none(),
        "a revoked invitation is no longer a live pending offer"
    );

    // With the prior offer revoked (and out of the partial index), a fresh invitation
    // to the same pair is seated.
    let reissued = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    create_invitation(&pool, &reissued).await;
    assert_eq!(
        find_pending(&pool, account.id, invitee).await.map(|i| i.id),
        Some(reissued.id),
        "re-inviting after a revoke seats a new pending offer"
    );
}

// An invitation id we never persisted resolves to nothing.
#[tokio::test]
async fn find_unknown_invitation_is_none() {
    let (pool, _container) = fresh_pool().await;
    let (account, inviter, invitee) = invitation_fixture(&pool, "ghost").await;

    // Issued in the domain but never persisted, so its id is genuinely unknown.
    let unstored = Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());

    let found = find_invitation(&pool, unstored.id).await;
    assert!(
        found.is_none(),
        "an invitation we never stored resolves to nothing"
    );
}

// Bug guard — `account_members`'s PRIMARY KEY is `(account_id, user_id)`, and the
// seat insert had no `ON CONFLICT` clause: accepting a still-pending invitation
// for a pair that's ALREADY seated (granted a role through another path while the
// offer sat pending) raised a PK violation and 500'd, leaving the invitation
// permanently pending (a repeatable 500). `ON CONFLICT DO NOTHING` makes the seat
// a no-op that keeps the ORIGINAL membership row.
#[tokio::test]
async fn accepting_an_invitation_for_an_already_seated_pair_is_a_no_op() {
    let (pool, _container) = fresh_pool().await;
    let (account, inviter, invitee) = invitation_fixture(&pool, "reseat").await;

    // The invitee is granted Admin directly, bypassing any invitation.
    grant_role(
        &pool,
        &UserAccount {
            account_id: account.id,
            user_id: invitee,
            role: Role::Admin(None),
        },
    )
    .await;

    // A stale pending invitation (issued before the grant) offers only Member.
    let invitation =
        Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());
    create_invitation(&pool, &invitation).await;

    // Accepting it must not error, and must not downgrade the already-granted role.
    let seated = accept_invitation(&pool, invitation, false).await;
    assert_eq!(
        seated.role,
        Role::Admin(None),
        "the returned membership reflects the original grant, not the invitation's role"
    );
    assert_eq!(
        role_of(&pool, invitee, account.id).await,
        Some(Role::Admin(None)),
        "the persisted membership still holds the original grant"
    );
    assert_eq!(
        parent_of(&pool, account.id, invitee).await,
        None,
        "the row is untouched entirely — parent stays NULL from the direct \
         grant, not overwritten to this invitation's inviter"
    );
}

// --- ZMVP-21: leaving an account ---

/// Reads `account_members.parent` directly — no port exposes it, and the re-homing
/// tests need to assert the role-tree edge. Unchecked (runtime) query, so it needs
/// no offline cache entry.
async fn parent_of(pool: &PgPool, account: AccountId, user: UserId) -> Option<uuid::Uuid> {
    sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "SELECT parent FROM account_members WHERE account_id = $1 AND user_id = $2",
    )
    .bind(*account)
    .bind(*user)
    .fetch_one(pool)
    .await
    .expect("read parent")
}

/// Seats `invited` as a Member under `inviter` (`parent = inviter`) by issuing and
/// accepting an invitation — the only path that writes `account_members.parent`.
async fn seat_under(pool: &PgPool, account: AccountId, invited: UserId, inviter: UserId) {
    let invitation = Invitation::issue(account, invited, Role::Member(None), inviter, Utc::now());
    create_invitation(pool, &invitation).await;
    accept_invitation(pool, invitation, true).await;
}

// AC3 — when a leaving member has children in the role tree, those children re-home
// to the leaver's Parent (DESIGN/Roles rule 3), and the leaver holds no role after.
#[tokio::test]
async fn leave_rehomes_children_to_the_leavers_parent() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:rehome-o").await;
    let a = provision(&pool, "did:plc:rehome-a").await;
    let b = provision(&pool, "did:plc:rehome-b").await;
    let c = provision(&pool, "did:plc:rehome-c").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rehome-acct".to_string()),
        "rehome-acct.example.com".parse::<Handle>().unwrap(),
        "Tree".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    seat_under(&pool, account.id, a.id, owner.id).await; // A's parent is the Owner
    seat_under(&pool, account.id, b.id, a.id).await; // B's parent is A
    seat_under(&pool, account.id, c.id, a.id).await; // C's parent is A

    leave(&pool, a.id, account.id).await; // A leaves

    assert_eq!(
        parent_of(&pool, account.id, b.id).await,
        Some(*owner.id),
        "B re-homes to A's parent (the Owner)"
    );
    assert_eq!(
        parent_of(&pool, account.id, c.id).await,
        Some(*owner.id),
        "C re-homes to A's parent (the Owner)"
    );
    assert_eq!(
        role_of(&pool, a.id, account.id).await,
        None,
        "after leaving, the member holds no role"
    );
}

// Bug guard — re-homing is scoped to the account being left. `parent` is a
// `users(id)`, so a member who parents children in two accounts must only have the
// left account's tree touched.
#[tokio::test]
async fn leave_is_scoped_to_the_account_being_left() {
    let (pool, _container) = fresh_pool().await;

    let o1 = provision(&pool, "did:plc:scope-o1").await;
    let o2 = provision(&pool, "did:plc:scope-o2").await;
    let a = provision(&pool, "did:plc:scope-a").await;
    let b = provision(&pool, "did:plc:scope-b").await;
    let d = provision(&pool, "did:plc:scope-d").await;

    let (acct1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:scope-acct1".to_string()),
        "scope-acct1.example.com".parse::<Handle>().unwrap(),
        "One".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &acct1, &m1).await;
    let (acct2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:scope-acct2".to_string()),
        "scope-acct2.example.com".parse::<Handle>().unwrap(),
        "Two".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &acct2, &m2).await;

    // A parents B in acct1 and D in acct2.
    seat_under(&pool, acct1.id, a.id, o1.id).await;
    seat_under(&pool, acct1.id, b.id, a.id).await;
    seat_under(&pool, acct2.id, a.id, o2.id).await;
    seat_under(&pool, acct2.id, d.id, a.id).await;

    leave(&pool, a.id, acct1.id).await; // A leaves acct1

    assert_eq!(
        parent_of(&pool, acct1.id, b.id).await,
        Some(*o1.id),
        "B re-homes in the left account"
    );
    assert_eq!(
        parent_of(&pool, acct2.id, d.id).await,
        Some(*a.id),
        "D in the other account is untouched — leaving is account-scoped"
    );
    assert!(
        role_of(&pool, a.id, acct2.id).await.is_some(),
        "A is still a member of the account they didn't leave"
    );
}

// ZMVP-40 — leaving revokes the leaver's still-pending *issued* invitations (so none
// later seats a member under a non-member), and only those.
#[tokio::test]
async fn leave_revokes_the_leavers_pending_issued_invitations() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:rev-o").await;
    let a = provision(&pool, "did:plc:rev-a").await;
    let x = provision(&pool, "did:plc:rev-x").await;
    let y = provision(&pool, "did:plc:rev-y").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rev-acct".to_string()),
        "rev-acct.example.com".parse::<Handle>().unwrap(),
        "Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    seat_under(&pool, account.id, a.id, owner.id).await;

    // A (leaving) has a pending offer out to X; the Owner (staying) has one out to Y.
    let a_invites_x = Invitation::issue(account.id, x.id, Role::Member(None), a.id, Utc::now());
    create_invitation(&pool, &a_invites_x).await;
    let owner_invites_y =
        Invitation::issue(account.id, y.id, Role::Member(None), owner.id, Utc::now());
    create_invitation(&pool, &owner_invites_y).await;

    leave(&pool, a.id, account.id).await; // A leaves

    let a_offer = find_invitation(&pool, a_invites_x.id)
        .await
        .expect("A's offer still exists as a record");
    assert_eq!(
        a_offer.state,
        InvitationState::Revoked,
        "the leaver's issued offer is revoked, not deleted"
    );
    assert!(
        find_pending(&pool, account.id, y.id).await.is_some(),
        "an offer issued by someone still present stays pending"
    );
}

// ZMVP-40 + Roles rule 3 — `revoke_role` is a member-departure event with the same
// store effects as `leave`: it re-homes the removed member's children to their
// parent AND revokes the member's pending issued invitations (they share
// `settle_member_departure`).
#[tokio::test]
async fn revoke_role_rehomes_children_and_revokes_issued_invitations() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:rv-o").await;
    let a = provision(&pool, "did:plc:rv-a").await;
    let b = provision(&pool, "did:plc:rv-b").await;
    let x = provision(&pool, "did:plc:rv-x").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rv-acct".to_string()),
        "rv-acct.example.com".parse::<Handle>().unwrap(),
        "Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    seat_under(&pool, account.id, a.id, owner.id).await; // A under the Owner
    seat_under(&pool, account.id, b.id, a.id).await; // B under A

    // A has a pending offer out to X.
    let a_invites_x = Invitation::issue(account.id, x.id, Role::Member(None), a.id, Utc::now());
    create_invitation(&pool, &a_invites_x).await;

    // An Owner/Admin revokes A's role (authority is the handler's; the store settles).
    revoke_role(&pool, a.id, account.id).await;

    assert_eq!(
        parent_of(&pool, account.id, b.id).await,
        Some(*owner.id),
        "B re-homes to A's parent (rule 3)"
    );
    assert_eq!(
        role_of(&pool, a.id, account.id).await,
        None,
        "the revoked member holds no role"
    );
    let offer = find_invitation(&pool, a_invites_x.id)
        .await
        .expect("the offer still exists as a record");
    assert_eq!(
        offer.state,
        InvitationState::Revoked,
        "the revoked member's issued offer is revoked, not deleted"
    );
}

// ── ZMVP-44 handle uniqueness ───────────────────────────────────────────────────

// The `accounts_handle_key` unique index rejects a second account claiming a
// handle another account already holds — and the pg adapter maps that violation to
// the typed `HandleTaken` (which the founding handler answers as 409), never a
// silent second row or a raw 500.
#[tokio::test]
async fn create_rejects_a_duplicate_handle() {
    let (pool, _container) = fresh_pool().await;
    let o1 = provision(&pool, "did:plc:dup-o1").await;
    let o2 = provision(&pool, "did:plc:dup-o2").await;

    let (a1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:dup-a1".to_string()),
        "dup.zurfur.app".parse::<Handle>().unwrap(),
        "One".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    // A different account (its own did/id) claiming the same handle is rejected.
    let (a2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:dup-a2".to_string()),
        "dup.zurfur.app".parse::<Handle>().unwrap(),
        "Two".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let err = try_create(&pool, &a2, &m2)
        .await
        .expect_err("a duplicate handle is rejected");
    assert!(
        err.downcast_ref::<HandleTaken>().is_some(),
        "the collision maps to HandleTaken (→409), got: {err:?}"
    );
}

// A soft-deleted (tombstoned) account still reserves its handle: it is invisible to
// resolution (`find_did_by_handle` → None) yet founding over its handle still fails
// with `HandleTaken`. The index is GLOBAL, not filtered on deleted_at — DD 23003138
// "Account Deletion, Tombstoning & Handle Reuse". This is the case the founding
// handler's live pre-check cannot see, so the constraint is the authoritative 409.
#[tokio::test]
async fn a_soft_deleted_account_still_reserves_its_handle() {
    let (pool, _container) = fresh_pool().await;
    let o1 = provision(&pool, "did:plc:ts-o1").await;
    let o2 = provision(&pool, "did:plc:ts-o2").await;

    let handle = "reserved.zurfur.app".parse::<Handle>().unwrap();
    let (a1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:ts-a1".to_string()),
        handle.clone(),
        "Gone".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    // Soft-delete through the real write path (ZMVP-34's `AccountWrites::soft_delete`).
    soft_delete(&pool, a1.id).await;

    // Invisible to resolution — the resolver would 404.
    assert!(
        find_did_by_handle(&pool, &handle).await.is_none(),
        "a tombstoned handle does not resolve"
    );

    // ...but the handle is still reserved: founding over it fails with HandleTaken,
    // which the handler answers 409 (not 500).
    let (a2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:ts-a2".to_string()),
        handle.clone(),
        "Reclaim".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let err = try_create(&pool, &a2, &m2)
        .await
        .expect_err("the tombstoned handle is still reserved");
    assert!(
        err.downcast_ref::<HandleTaken>().is_some(),
        "reserved-by-tombstone maps to HandleTaken (→409), got: {err:?}"
    );
}

// --- ZMVP-33: transferring ownership ---

/// Transfer ownership in one unit of work (the way the handler does).
async fn transfer_ownership(
    pool: &PgPool,
    old_owner: UserId,
    new_owner: UserId,
    account: AccountId,
) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .transfer_ownership(old_owner, new_owner, account)
        .await
        .expect("transfer_ownership");
    uow.commit().await.expect("commit");
}

// ACs 1–3 + Roles rule 5, against real SQL: after a transfer the named member is the
// sole Owner with no parent, and the prior Owner is an Admin re-homed under the new
// Owner. The `parent` edges are the store's job the mem fake can't model, so they're
// proven here.
#[tokio::test]
async fn transfer_makes_the_heir_owner_and_demotes_the_prior_owner_to_admin() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:xfer-o").await;
    let heir = provision(&pool, "did:plc:xfer-h").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:xfer-acct".to_string()),
        "xfer-acct.example.com".parse::<Handle>().unwrap(),
        "Hand-Off".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    // Seat the heir as a Member under the Owner (parent = Owner) before the transfer.
    seat_under(&pool, account.id, heir.id, owner.id).await;

    transfer_ownership(&pool, owner.id, heir.id, account.id).await;

    assert_eq!(
        role_of(&pool, heir.id, account.id).await,
        Some(Role::Owner(None)),
        "the heir is the new Owner",
    );
    assert_eq!(
        parent_of(&pool, account.id, heir.id).await,
        None,
        "an Owner never has a parent (Roles rule 5)",
    );
    assert_eq!(
        role_of(&pool, owner.id, account.id).await,
        Some(Role::Admin(None)),
        "the prior Owner is demoted to Admin",
    );
    assert_eq!(
        parent_of(&pool, account.id, owner.id).await,
        Some(*heir.id),
        "the outgoing Owner is re-homed under the new Owner (Roles rule 8)",
    );
}

// Backstop: the outgoing Owner must actually be the Owner. Handing off from a
// non-Owner errors and — being one unit of work — leaves the roster untouched.
#[tokio::test]
async fn transfer_from_a_non_owner_errors_and_changes_nothing() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:nonowner-o").await;
    let admin = provision(&pool, "did:plc:nonowner-a").await;
    let heir = provision(&pool, "did:plc:nonowner-h").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:nonowner-acct".to_string()),
        "nonowner-acct.example.com".parse::<Handle>().unwrap(),
        "Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    seat_under(&pool, account.id, admin.id, owner.id).await;
    seat_under(&pool, account.id, heir.id, owner.id).await;

    // `admin` is not the Owner, so the store guard rejects the transfer.
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let result = uow
        .accounts()
        .transfer_ownership(admin.id, heir.id, account.id)
        .await;
    assert!(result.is_err(), "a non-Owner cannot transfer ownership");
    drop(uow); // roll the unit back

    assert_eq!(
        role_of(&pool, owner.id, account.id).await,
        Some(Role::Owner(None)),
        "the real Owner still owns the account",
    );
    assert_eq!(
        role_of(&pool, heir.id, account.id).await,
        Some(Role::Member(None)),
        "the would-be heir's role is unchanged",
    );
}

// Backstop: the incoming Owner must actually be a member. When the promotion guard
// matches zero rows the whole unit rolls back — the demotion is undone, so the
// original Owner still owns the account (never left ownerless).
#[tokio::test]
async fn transfer_to_a_non_member_errors_and_keeps_the_owner() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:nonmember-o").await;
    let stranger = provision(&pool, "did:plc:nonmember-s").await; // provisioned, never seated

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:nonmember-acct".to_string()),
        "nonmember-acct.example.com".parse::<Handle>().unwrap(),
        "Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let result = uow
        .accounts()
        .transfer_ownership(owner.id, stranger.id, account.id)
        .await;
    assert!(result.is_err(), "cannot transfer ownership to a non-member");
    drop(uow); // roll the unit back

    assert_eq!(
        role_of(&pool, owner.id, account.id).await,
        Some(Role::Owner(None)),
        "the demotion rolled back with the failed promotion — the Owner is intact",
    );
    assert_eq!(
        role_of(&pool, stranger.id, account.id).await,
        None,
        "the non-member gained no role",
    );
}

// A hard-deleted (empty) account is gone: its row and Owner membership are removed, so
// `find`/`role_of` read None — and, UNLIKE a soft-delete, its handle is FREED, so a
// brand-new, different account may claim it. Hard-delete is only for empty accounts, so
// the freed label carries no reputation (DD 23003138).
#[tokio::test]
async fn hard_delete_frees_the_handle_for_reuse() {
    let (pool, _container) = fresh_pool().await;
    let store = PgAccountStore::new(pool.clone());
    let o1 = provision(&pool, "did:plc:hd-o1").await;
    let o2 = provision(&pool, "did:plc:hd-o2").await;

    let handle = "freed.zurfur.app".parse::<Handle>().unwrap();
    let (a1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:hd-a1".to_string()),
        handle.clone(),
        "Empty".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    hard_delete(&pool, a1.id).await;

    // The account and its Owner membership are gone.
    assert!(
        store.find(a1.id).await.expect("find").is_none(),
        "a hard-deleted account is gone"
    );
    assert!(
        store
            .role_of(o1.id, a1.id)
            .await
            .expect("role_of")
            .is_none(),
        "the Owner membership went with it"
    );

    // The handle is FREED — a new, different account may now claim it (contrast the
    // soft-delete case above, where the handle stays reserved).
    let (a2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:hd-a2".to_string()),
        handle.clone(),
        "Reclaimed".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    try_create(&pool, &a2, &m2)
        .await
        .expect("the freed handle may be reclaimed by a new account");
    assert_eq!(
        find_did_by_handle(&pool, &handle).await,
        Some(a2.did.clone()),
        "the handle now resolves to the new account"
    );
}

// Hard-delete also removes the account's pending invitations, not just its row and
// memberships — nothing is left dangling to reference a deleted account.
#[tokio::test]
async fn hard_delete_removes_pending_invitations() {
    let (pool, _container) = fresh_pool().await;
    let store = PgAccountStore::new(pool.clone());
    let owner = provision(&pool, "did:plc:hi-o").await;
    let invitee = provision(&pool, "did:plc:hi-i").await;

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:hi-a".to_string()),
        "invited.zurfur.app".parse::<Handle>().unwrap(),
        "Has Invite".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;

    let invitation = Invitation::issue(
        account.id,
        invitee.id,
        Role::Member(None),
        owner.id,
        Utc::now(),
    );
    create_invitation(&pool, &invitation).await;
    assert!(
        store
            .find_pending_invitation(account.id, invitee.id)
            .await
            .expect("find_pending_invitation")
            .is_some(),
        "the invitation is pending before the delete"
    );

    hard_delete(&pool, account.id).await;

    assert!(
        store
            .find_invitation(invitation.id)
            .await
            .expect("find_invitation")
            .is_none(),
        "the pending invitation is removed with the account"
    );
}

// ── ZMVP-46 handle change ───────────────────────────────────────────────────────
// Round-trips the change flow's private half against real SQL: the `accounts.handle`
// repoint (so resolution follows), the `account_handle_changes` audit row that backs
// the rate limit and quarantine, and the HandleTaken mapping on a collision. DD 27852802.

/// Change an account's handle in one unit of work, returning the result (so a test can
/// assert an error). Commits only on success — a failed unit rolls back on drop.
async fn try_change_handle(
    pool: &PgPool,
    account: AccountId,
    old: &Handle,
    new: &Handle,
    at: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let result = uow.accounts().change_handle(account, old, new, at).await;
    if result.is_ok() {
        uow.commit().await.expect("commit");
    }
    result
}

/// Count an account's recorded handle changes since `since`, off the pool-backed store.
async fn count_changes(pool: &PgPool, account: AccountId, since: chrono::DateTime<Utc>) -> i64 {
    PgAccountStore::new(pool.clone())
        .count_handle_changes_since(account, since)
        .await
        .expect("count_handle_changes_since")
}

/// Whether `handle` is quarantined to an account other than `excluding` since `since`.
async fn reserved_for_other(
    pool: &PgPool,
    handle: &Handle,
    excluding: Option<AccountId>,
    since: chrono::DateTime<Utc>,
) -> bool {
    PgAccountStore::new(pool.clone())
        .handle_reserved_for_other(handle, excluding, since)
        .await
        .expect("handle_reserved_for_other")
}

// The private half of a change: `accounts.handle` repoints (so resolution follows), and
// the audit row lands (so the change is counted). One unit of work.
#[tokio::test]
async fn change_handle_repoints_resolution_and_records_the_change() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:chg-o").await;

    let old = "chg-before.zurfur.app".parse::<Handle>().unwrap();
    let new = "chg-after.zurfur.app".parse::<Handle>().unwrap();
    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:chg-acct".to_string()),
        old.clone(),
        "Rename".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;

    let before = Utc::now();
    try_change_handle(&pool, account.id, &old, &new, Utc::now())
        .await
        .expect("the change commits");

    // handle→DID resolution followed: the new handle resolves, the old does not.
    assert_eq!(
        find_did_by_handle(&pool, &new).await,
        Some(account.did.clone()),
        "the new handle resolves to the account's DID"
    );
    assert!(
        find_did_by_handle(&pool, &old).await.is_none(),
        "the old handle no longer resolves"
    );
    // The account row carries the new handle.
    assert_eq!(
        find_account(&pool, account.id).await.expect("live").handle,
        new,
        "the account's stored handle is the new one"
    );
    // The change was recorded (backs the rate limit).
    assert_eq!(
        count_changes(&pool, account.id, before).await,
        1,
        "the change is recorded exactly once"
    );
}

// A change to a handle another live account already holds fails with the typed
// `HandleTaken` (→ 409), and — being one unit of work — leaves the account unchanged.
#[tokio::test]
async fn change_handle_rejects_a_taken_handle() {
    let (pool, _container) = fresh_pool().await;
    let o1 = provision(&pool, "did:plc:chgdup-o1").await;
    let o2 = provision(&pool, "did:plc:chgdup-o2").await;

    let mine = "chgdup-mine.zurfur.app".parse::<Handle>().unwrap();
    let (a1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:chgdup-a1".to_string()),
        mine.clone(),
        "Mine".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    let theirs = "chgdup-theirs.zurfur.app".parse::<Handle>().unwrap();
    let (a2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:chgdup-a2".to_string()),
        theirs.clone(),
        "Theirs".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &a2, &m2).await;

    let err = try_change_handle(&pool, a1.id, &mine, &theirs, Utc::now())
        .await
        .expect_err("changing to a taken handle is rejected");
    assert!(
        err.downcast_ref::<HandleTaken>().is_some(),
        "the collision maps to HandleTaken (→409), got: {err:?}"
    );
    // Rolled back: a1 still holds its original handle.
    assert_eq!(
        find_account(&pool, a1.id).await.expect("live").handle,
        mine,
        "a rejected change leaves the account's handle untouched"
    );
}

// Optimistic concurrency (Copilot review): `old` is a precondition, not just an
// observation. A change against a stale `old` (not the row's current handle) matches no
// row, fails, and — being one unit of work — records NO audit entry, so the log can
// never capture a wrong `old_handle` that would leave the truly vacated handle
// un-quarantined. The account keeps its real handle.
#[tokio::test]
async fn change_handle_rejects_a_stale_old_handle() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:stale-o").await;

    let current = "stale-current.zurfur.app".parse::<Handle>().unwrap();
    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:stale-acct".to_string()),
        current.clone(),
        "Stale".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;

    // A caller that observed a DIFFERENT (stale) old handle tries to change to a new one.
    let stale = "stale-wrong.zurfur.app".parse::<Handle>().unwrap();
    let new = "stale-new.zurfur.app".parse::<Handle>().unwrap();
    let before = Utc::now();
    let err = try_change_handle(&pool, account.id, &stale, &new, Utc::now())
        .await
        .expect_err("a change against a stale old handle is rejected");
    assert!(
        err.downcast_ref::<HandleTaken>().is_none(),
        "a stale precondition is not a HandleTaken — it's a rolled-back no-change"
    );
    // Nothing changed: the account keeps its real handle and no audit row was written.
    assert_eq!(
        find_account(&pool, account.id).await.expect("live").handle,
        current,
        "the account keeps its actual handle after a rejected stale change"
    );
    assert_eq!(
        count_changes(&pool, account.id, before).await,
        0,
        "no audit row is recorded for a rejected stale change"
    );
}

// ── ZMVP-57: account-anchored fact gate & positioning severance ──────────────────

/// AC4 — THE ACCOUNT-FACT TRIPWIRE (mirrors ZMVP-67's commission tripwire): every
/// table holding a foreign key onto `accounts(id)` must be **deliberately
/// classified** — registered in [`ACCOUNT_FACT_TABLES`] (its rows are
/// account-anchored facts; an account bearing one is soft-deleted, never hard) or in
/// [`ACCOUNT_NON_FACT_TABLES`] (bookkeeping severed with the account, never a fact).
/// A migration that adds an `accounts`-referencing table trips this test until its
/// author makes that call in code — and registering a fact table trips the
/// compile-time guards in `adapter_pg::account` and beside `account_has_facts` in the
/// `api` crate, which refuse to build until that seam stops returning its unexamined
/// constant `false`. Neither step can happen by accident (Account Deletion DD 23003138).
#[tokio::test]
async fn every_account_referencing_table_is_classified_as_fact_or_non_fact() {
    let (pool, _container) = fresh_pool().await;

    // Every table holding a foreign key onto `accounts` — the schema-level superset
    // of possible account-anchored storage.
    let referencing: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT conrelid::regclass::text
        FROM pg_constraint
        WHERE contype = 'f' AND confrelid = 'accounts'::regclass
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("scan foreign keys onto accounts");

    let referencing: BTreeSet<&str> = referencing.iter().map(String::as_str).collect();
    let facts: BTreeSet<&str> = ACCOUNT_FACT_TABLES.iter().copied().collect();
    let non_facts: BTreeSet<&str> = ACCOUNT_NON_FACT_TABLES.iter().copied().collect();

    let overlap: Vec<&&str> = facts.intersection(&non_facts).collect();
    assert!(
        overlap.is_empty(),
        "a table cannot be both fact and non-fact: {overlap:?}"
    );

    // AC2 (structural) — commissions are NOT account facts (Ownership Separation DD
    // 29130754): a commission is User-owned and carries no foreign key onto
    // `accounts`, so it can never enter this scan and can never be wired into
    // `account_has_facts`. Assert that structural fact directly.
    assert!(
        !referencing.contains("commission"),
        "commission must not reference accounts(id) — it is User-owned and survives \
         account deletion, never an account-anchored fact",
    );

    let classified: BTreeSet<&str> = facts.union(&non_facts).copied().collect();
    assert_eq!(
        referencing, classified,
        "every table referencing accounts(id) must be listed in exactly one of \
         ACCOUNT_FACT_TABLES (the account_has_facts registry, Account Deletion DD 23003138) \
         or ACCOUNT_NON_FACT_TABLES (deliberate exemptions) in adapter-pg/src/account.rs — \
         classify it there, in the same change that adds the table"
    );
}

/// AC1 — account hard-delete **severs** the account's positioning rails (its
/// placements and view grants) while the placed **commission survives untouched**.
/// Commissions are User-owned, not account facts (Ownership Separation DD 29130754),
/// so a placed commission never forces a soft-delete; severance rides the ZMVP-70
/// `ON DELETE CASCADE` on each positioning FK onto `accounts`, exercised through the
/// real [`AccountWrites::hard_delete`](domain::ports::AccountWrites::hard_delete) path.
#[tokio::test]
async fn hard_delete_severs_placements_and_grants_while_the_commission_survives() {
    let (pool, _container) = fresh_pool().await;
    let commissions = PgCommissionStore::new(pool.clone());
    let accounts = PgAccountStore::new(pool.clone());

    // A User owns a commission; a separate account will hold its positioning.
    let owner = provision(&pool, "did:plc:sever-owner").await;
    let commission = Commission::create(
        "A ref sheet".parse::<CommissionTitle>().expect("title"),
        owner.id,
        Utc::now(),
        None,
    );
    let commission_id = commission.id;
    {
        let db = PgDatabase::new(pool.clone());
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .create(&commission)
            .await
            .expect("create commission");
        uow.commit().await.expect("commit");
    }

    let account_owner = provision(&pool, "did:plc:sever-acct-owner").await;
    let (account, membership) = Account::open(
        account_owner.id,
        Did::new("did:plc:sever-acct".to_string()),
        "sever-acct.zurfur.app".parse::<Handle>().unwrap(),
        "Holder".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let account_id = account.id;
    create(&pool, &account, &membership).await;

    // Place the commission in the account's position and grant it a Total view key.
    {
        let db = PgDatabase::new(pool.clone());
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .place(commission_id, account_id, owner.id, Utc::now())
            .await
            .expect("place");
        uow.commissions()
            .grant_view(commission_id, account_id, GrantLevel::Total)
            .await
            .expect("grant");
        uow.commit().await.expect("commit");
    }
    // Precondition: the positioning rails exist before the delete.
    assert!(
        commissions
            .current_placement(commission_id)
            .await
            .unwrap()
            .is_some(),
        "the commission is placed before the delete"
    );
    assert!(
        !commissions
            .placement_log(commission_id)
            .await
            .unwrap()
            .is_empty(),
        "the placement log has a row before the delete"
    );
    assert!(
        commissions
            .view_grant(commission_id, account_id)
            .await
            .unwrap()
            .is_some(),
        "the view grant exists before the delete"
    );

    // Hard-delete the account: it holds no account-anchored fact (a placed commission
    // is not one), so it takes the hard path.
    hard_delete(&pool, account_id).await;

    // The account is gone...
    assert!(
        accounts.find(account_id).await.expect("find").is_none(),
        "the account is hard-deleted"
    );
    // ...its positioning rails are severed...
    assert!(
        commissions
            .current_placement(commission_id)
            .await
            .unwrap()
            .is_none(),
        "the current-placement pointer is severed with the account"
    );
    assert!(
        commissions
            .placement_log(commission_id)
            .await
            .unwrap()
            .is_empty(),
        "the placement log is severed with the account"
    );
    assert!(
        commissions
            .view_grant(commission_id, account_id)
            .await
            .unwrap()
            .is_none(),
        "the view grant is severed with the account"
    );
    // ...but the commission itself survives untouched.
    let survivor = commissions
        .find(commission_id)
        .await
        .expect("find")
        .expect("the User-owned commission survives account deletion");
    assert_eq!(
        survivor.id, commission_id,
        "the commission is untouched by account deletion"
    );
    assert_eq!(
        survivor.owner_id, owner.id,
        "its ownership is unchanged — the commission never belonged to the account"
    );
}

// The quarantine read: a vacated handle is reserved to the account that left it —
// visible to *others* (§4), excluded for that account itself (reclaimable), and no
// longer matching once the window has passed.
#[tokio::test]
async fn quarantine_reserves_the_vacated_handle_to_the_leaving_account() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:quar-o").await;

    let vacated = "quar-vacated.zurfur.app".parse::<Handle>().unwrap();
    let moved_to = "quar-moved.zurfur.app".parse::<Handle>().unwrap();
    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:quar-acct".to_string()),
        vacated.clone(),
        "Quar".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(&pool, &account, &membership).await;
    try_change_handle(&pool, account.id, &vacated, &moved_to, Utc::now())
        .await
        .expect("change commits");

    let window = Utc::now() - Duration::days(30);
    // Some OTHER account is barred from the vacated handle.
    let stranger = AccountId::new(uuid::Uuid::now_v7());
    assert!(
        reserved_for_other(&pool, &vacated, Some(stranger), window).await,
        "the vacated handle is quarantined to its former holder — barred to others"
    );
    // The account that vacated it may reclaim it (excluded from its own quarantine).
    assert!(
        !reserved_for_other(&pool, &vacated, Some(account.id), window).await,
        "the leaving account may reclaim its own quarantined handle"
    );
    // Once the window has passed (a floor in the future), the reservation lifts.
    assert!(
        !reserved_for_other(
            &pool,
            &vacated,
            Some(stranger),
            Utc::now() + Duration::days(1)
        )
        .await,
        "an expired quarantine no longer reserves the handle"
    );
}

// --- ZMVP-157: `list_for_user` against real SQL. ---
//
// These matter more than most round-trip tests: `WHERE am.user_id = $1 AND
// a.deleted_at IS NULL AND (am.listed_on_profile OR NOT $2)` IS the
// authorization boundary for `GET /accounts` — the only thing standing between
// a caller and other users' rows. The API-level suites exercise adapter-mem,
// whose implementation is independently hand-written, so a divergence in a
// scoping predicate is exactly the regression those tests cannot catch.

/// Read a user's own accounts off the pool-backed store.
async fn list_for_user(pool: &PgPool, user: UserId, scope: ListingScope) -> Vec<AccountMembership> {
    PgAccountStore::new(pool.clone())
        .list_for_user(user, scope)
        .await
        .expect("list_for_user")
}

/// Found an account owned by `owner_did`, returning it. Each call mints its own
/// DID/handle from `tag`, so a test can seed several without collisions.
async fn found_account(pool: &PgPool, owner: UserId, tag: &str) -> Account {
    let (account, membership) = Account::open(
        owner,
        Did::new(format!("did:plc:pglist-{tag}")),
        format!("pglist-{tag}.example.com")
            .parse::<Handle>()
            .unwrap(),
        "PG Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    create(pool, &account, &membership).await;
    account
}

// The listing is every live account the caller holds a role in — founded AND
// granted alike (Engineer ruling 2026-07-24) — each row carrying the caller's
// OWN role, ordered by id. The stranger's account is the containment case: it
// proves the `user_id` predicate scopes the query rather than merely decorating
// it, which is the property ZMVP-17 rests on.
#[tokio::test]
async fn list_for_user_returns_every_live_membership_with_the_callers_own_role() {
    let (pool, _container) = fresh_pool().await;

    let caller = provision(&pool, "did:plc:pglist-caller").await;
    let stranger = provision(&pool, "did:plc:pglist-stranger").await;

    let owned = found_account(&pool, caller.id, "owned").await;

    // A second account someone else founded, on which the caller holds a
    // non-Owner role — the accepted-invitation shape (ZMVP-20).
    let joined = found_account(&pool, stranger.id, "joined").await;
    let seat = UserAccount {
        user_id: caller.id,
        account_id: joined.id,
        role: Role::Member(None),
    };
    grant_role(&pool, &seat).await;

    // A third the caller has nothing to do with: must never appear.
    let theirs = found_account(&pool, stranger.id, "theirs").await;

    let rows = list_for_user(&pool, caller.id, ListingScope::SelfView).await;

    let listed: Vec<AccountId> = rows.iter().map(|row| row.account.id).collect();
    assert!(
        !listed.contains(&theirs.id),
        "an account the caller holds no role in is never listed, got {listed:?}"
    );
    assert_eq!(
        listed.len(),
        2,
        "exactly the two memberships, got {listed:?}"
    );

    let owned_row = rows
        .iter()
        .find(|row| row.account.id == owned.id)
        .expect("the founded account is listed");
    assert_eq!(
        owned_row.role,
        Role::Owner(None),
        "the founder's own role rides along"
    );
    assert_eq!(
        owned_row.account.handle, owned.handle,
        "the row is hydrated"
    );
    assert_eq!(owned_row.account.did, owned.did, "the DID join resolves");

    let joined_row = rows
        .iter()
        .find(|row| row.account.id == joined.id)
        .expect("the granted-only account is listed too — not owned-only");
    assert_eq!(
        joined_row.role,
        Role::Member(None),
        "the CALLER's role, not the account owner's"
    );

    // Ordering is `ORDER BY a.id`, and account ids are UUIDv7 — creation order.
    let order: Vec<uuid::Uuid> = listed.iter().map(|id| **id).collect();
    let mut expected = order.clone();
    expected.sort();
    assert_eq!(order, expected, "rows come back ascending by account id");
}

// Soft-deleted accounts are absent, mirroring `find`'s `deleted_at IS NULL`
// liveness filter (DD 23003138). The membership row survives the tombstone, so
// only the join's predicate keeps it out of the listing.
#[tokio::test]
async fn list_for_user_excludes_a_soft_deleted_account() {
    let (pool, _container) = fresh_pool().await;

    let caller = provision(&pool, "did:plc:pglist-live-caller").await;
    let live = found_account(&pool, caller.id, "live").await;
    let doomed = found_account(&pool, caller.id, "doomed").await;

    soft_delete(&pool, doomed.id).await;

    let rows = list_for_user(&pool, caller.id, ListingScope::SelfView).await;
    let listed: Vec<AccountId> = rows.iter().map(|row| row.account.id).collect();
    assert_eq!(
        listed,
        vec![live.id],
        "only the live account is listed; the tombstoned one is filtered out"
    );
    assert!(
        role_of(&pool, caller.id, doomed.id).await.is_some(),
        "the membership row itself survives the soft delete — only the join filters it"
    );
}

// The `listed_on_profile` valve (DD 21594113 decision 4, Engineer ruling
// 2026-07-25) is honored for a PUBLIC projection and deliberately ignored for
// the member's own view. This is the test that makes `ListingScope` more than a
// parameter: the same user, the same rows, two different answers.
#[tokio::test]
async fn list_for_user_honors_the_privacy_valve_only_for_a_public_projection() {
    let (pool, _container) = fresh_pool().await;

    let owner = provision(&pool, "did:plc:pgvalve-owner").await;
    let member = provision(&pool, "did:plc:pgvalve-member").await.id;

    // Two accounts the member joins by invitation — one published, one not.
    let published = found_account(&pool, owner.id, "published").await;
    let unlisted = found_account(&pool, owner.id, "unlisted").await;

    for (account, listed) in [(&published, true), (&unlisted, false)] {
        let invitation =
            Invitation::issue(account.id, member, Role::Member(None), owner.id, Utc::now());
        create_invitation(&pool, &invitation).await;
        accept_invitation(&pool, invitation, listed).await;
    }

    let own_view = list_for_user(&pool, member, ListingScope::SelfView).await;
    let own_ids: BTreeSet<uuid::Uuid> = own_view.iter().map(|row| *row.account.id).collect();
    assert_eq!(
        own_ids,
        BTreeSet::from([*published.id, *unlisted.id]),
        "a member's OWN view shows every live membership — their publication \
         choice does not hide their records from themselves"
    );

    let public_view = list_for_user(&pool, member, ListingScope::PublicProfile).await;
    let public_ids: Vec<AccountId> = public_view.iter().map(|row| row.account.id).collect();
    assert_eq!(
        public_ids,
        vec![published.id],
        "a PUBLIC projection shows only the memberships the member published"
    );
}
