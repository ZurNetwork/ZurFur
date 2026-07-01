//! Round-trips the account/membership/invitation store against a throwaway
//! PostgreSQL container, proving the migration-created `accounts` /
//! `account_members` / `account_invitations` tables persist a founded account and
//! its Owner membership in one transaction, that reads come back, and that the
//! Unit-of-Work seam commits across aggregates atomically (and rolls back a
//! dropped unit). Reads go through [`PgAccountStore`]; every write goes through the
//! [`PgDatabase`] factory's [`UnitOfWork`] (DD `24150017`). The founder is
//! provisioned first because `account_members.user_id` references `users(id)`.
//! Requires a container runtime socket (DOCKER_HOST honored).
use adapter_pg::{PgAccountStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        handle::Handle,
        invitation::{Invitation, InvitationId, InvitationState},
        role::Role,
        user::{User, UserId},
        user_account::UserAccount,
    },
    ports::{AccountStore, Database, HandleTaken},
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots a fresh database and runs migrations. The container is returned so the
/// caller keeps it alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");
    (pool, container)
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
    let account_handle = Handle::try_new("pgacct.example.com").unwrap();
    let account_name = AccountName::try_new("PG Studio").unwrap();
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
        Handle::try_new("multi-acct.example.com").unwrap(),
        AccountName::try_new("Multi Studio").unwrap(),
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
        Handle::try_new("rollback-acct.example.com").unwrap(),
        AccountName::try_new("Rollback").unwrap(),
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
        Handle::try_new("ghost.example.com").unwrap(),
        AccountName::try_new("Ghost").unwrap(),
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
        Handle::try_new("pgacct3.example.com").unwrap(),
        AccountName::try_new("PG Studio 3").unwrap(),
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
        Handle::try_new(format!("pgacct-{tag}.example.com")).unwrap(),
        AccountName::try_new("PG Studio").unwrap(),
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
        Handle::try_new("rehome-acct.example.com").unwrap(),
        AccountName::try_new("Tree").unwrap(),
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
        Handle::try_new("scope-acct1.example.com").unwrap(),
        AccountName::try_new("One").unwrap(),
        Utc::now(),
    );
    create(&pool, &acct1, &m1).await;
    let (acct2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:scope-acct2".to_string()),
        Handle::try_new("scope-acct2.example.com").unwrap(),
        AccountName::try_new("Two").unwrap(),
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
        Handle::try_new("rev-acct.example.com").unwrap(),
        AccountName::try_new("Studio").unwrap(),
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
        Handle::try_new("rv-acct.example.com").unwrap(),
        AccountName::try_new("Studio").unwrap(),
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
        Handle::try_new("dup.zurfur.app").unwrap(),
        AccountName::try_new("One").unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    // A different account (its own did/id) claiming the same handle is rejected.
    let (a2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:dup-a2".to_string()),
        Handle::try_new("dup.zurfur.app").unwrap(),
        AccountName::try_new("Two").unwrap(),
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

    let handle = Handle::try_new("reserved.zurfur.app").unwrap();
    let (a1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:ts-a1".to_string()),
        handle.clone(),
        AccountName::try_new("Gone").unwrap(),
        Utc::now(),
    );
    create(&pool, &a1, &m1).await;

    // Tombstone it directly (no soft-delete write path exists yet).
    sqlx::query("UPDATE accounts SET deleted_at = now() WHERE id = $1")
        .bind(*a1.id)
        .execute(&pool)
        .await
        .expect("soft-delete the account");

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
        AccountName::try_new("Reclaim").unwrap(),
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
