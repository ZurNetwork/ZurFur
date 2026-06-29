//! Round-trips `PgAccountRepo` against a throwaway PostgreSQL container, proving the
//! migration-created `accounts`/`account_members` tables persist a founded account and
//! its Owner membership in one transaction, that `find` reads the account back and hides
//! the unknown, and that `role_of` answers membership. The founder is provisioned through
//! `PgUserRepo` first because `account_members.user_id` references `users(id)`. Requires a
//! container runtime socket (DOCKER_HOST honored).
use adapter_pg::{PgAccountRepo, PgPool, PgUserRepo};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        invitation::{Invitation, InvitationState},
        role::Role,
        user::UserId,
    },
    ports::{AccountRepo, UserRepo},
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

#[tokio::test]
async fn create_persists_the_account_and_its_owner_membership() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    // The founder must exist: account_members.user_id references users(id).
    let owner = users
        .provision(&Did::new("did:plc:pgowner".to_string()))
        .await
        .expect("provision owner");

    let account_did = Did::new("did:plc:pgacct".to_string());
    let account_name = AccountName::try_new("PG Studio").unwrap();
    let (account, membership) = Account::open(
        owner.id,
        account_did.clone(),
        account_name.clone(),
        Utc::now(),
    );
    let account_id = account.id;
    accounts
        .create(&account, &membership)
        .await
        .expect("create founds the account and its membership");

    let found = accounts
        .find(account_id)
        .await
        .expect("find")
        .expect("the founded account is present");
    assert_eq!(found.id, account_id);
    assert_eq!(
        found.did, account_did,
        "the account's minted did round-trips"
    );
    assert_eq!(found.name, account_name, "the account's name round-trips");
    assert_eq!(found.deleted_at, None, "a freshly founded account is live");

    let role = accounts
        .role_of(owner.id, account_id)
        .await
        .expect("role_of");
    assert_eq!(
        role,
        Some(Role::Owner(None)),
        "the creating User is the account's Owner"
    );
}

#[tokio::test]
async fn find_unknown_account_is_none() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:pgowner2".to_string()))
        .await
        .expect("provision owner");
    // Founded in the domain but never persisted, so its id is genuinely unknown.
    let (unfounded, _) = Account::open(
        owner.id,
        Did::new("did:plc:ghost".to_string()),
        AccountName::try_new("Ghost").unwrap(),
        Utc::now(),
    );

    let found = accounts.find(unfounded.id).await.expect("find");
    assert!(
        found.is_none(),
        "an account we never founded resolves to nothing"
    );
}

#[tokio::test]
async fn role_of_non_member_is_none() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:pgowner3".to_string()))
        .await
        .expect("provision owner");
    let stranger = users
        .provision(&Did::new("did:plc:pgstranger".to_string()))
        .await
        .expect("provision stranger");

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:pgacct3".to_string()),
        AccountName::try_new("PG Studio 3").unwrap(),
        Utc::now(),
    );
    accounts
        .create(&account, &membership)
        .await
        .expect("create");

    let role = accounts
        .role_of(stranger.id, account.id)
        .await
        .expect("role_of");
    assert_eq!(role, None, "a user who is not a member holds no role");
}

// ── ZMVP-32 invitations ───────────────────────────────────────────────────────
// Round-trips the invitation methods against the migration-created
// `account_invitations` table (and its partial unique index), mirroring the
// adapter-mem contract suite. The invited user, inviter, and account are
// provisioned first because the table's foreign keys reference users(id)/accounts(id).

/// Provisions an owner and an invitee, founds an account, and returns the handles
/// the invitation tests share. The owner doubles as the inviter.
async fn invitation_fixture(pool: &PgPool, tag: &str) -> (PgAccountRepo, Account, UserId, UserId) {
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new(format!("did:plc:pginviter-{tag}")))
        .await
        .expect("provision owner");
    let invitee = users
        .provision(&Did::new(format!("did:plc:pginvitee-{tag}")))
        .await
        .expect("provision invitee");
    let (account, membership) = Account::open(
        owner.id,
        Did::new(format!("did:plc:pgacct-{tag}")),
        AccountName::try_new("PG Studio").unwrap(),
        Utc::now(),
    );
    accounts
        .create(&account, &membership)
        .await
        .expect("found account");

    (accounts, account, owner.id, invitee.id)
}

// AC3 — a freshly issued pending invitation round-trips: it's the pending offer
// found for its (account, invited_user) pair, with every fact intact.
#[tokio::test]
async fn create_then_find_pending_returns_the_invitation() {
    let (pool, _container) = fresh_pool().await;
    let (accounts, account, inviter, invitee) = invitation_fixture(&pool, "rt").await;

    let invitation = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    let id = invitation.id;
    accounts
        .create_invitation(&invitation)
        .await
        .expect("create_invitation");

    let found = accounts
        .find_pending_invitation(account.id, invitee)
        .await
        .expect("find_pending_invitation")
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
    let (accounts, account, inviter, invitee) = invitation_fixture(&pool, "dup").await;

    let first = Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());
    let second = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    accounts
        .create_invitation(&first)
        .await
        .expect("first create");
    accounts
        .create_invitation(&second)
        .await
        .expect("second create is a no-op, not an error");

    let found = accounts
        .find_pending_invitation(account.id, invitee)
        .await
        .expect("find_pending_invitation")
        .expect("a pending invitation remains");
    assert_eq!(
        found.id, first.id,
        "the first pending offer is the one kept"
    );
    assert!(
        accounts
            .find_invitation(second.id)
            .await
            .expect("find_invitation")
            .is_none(),
        "the duplicate issue stored nothing"
    );
}

// AC4 — revoking flips the offer to revoked (guarded UPDATE): it reads back revoked,
// is no longer the live pending offer, and the partial index lets a re-invite through.
#[tokio::test]
async fn revoke_invitation_flips_state_and_clears_the_pending_offer() {
    let (pool, _container) = fresh_pool().await;
    let (accounts, account, inviter, invitee) = invitation_fixture(&pool, "rev").await;

    let invitation =
        Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());
    let id = invitation.id;
    accounts
        .create_invitation(&invitation)
        .await
        .expect("create_invitation");

    accounts
        .revoke_invitation(id)
        .await
        .expect("revoke_invitation");

    assert_eq!(
        accounts
            .find_invitation(id)
            .await
            .expect("find_invitation")
            .map(|i| i.state),
        Some(InvitationState::Revoked),
        "the invitation reads back revoked"
    );
    assert!(
        accounts
            .find_pending_invitation(account.id, invitee)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a revoked invitation is no longer a live pending offer"
    );

    // With the prior offer revoked (and out of the partial index), a fresh invitation
    // to the same pair is seated.
    let reissued = Invitation::issue(account.id, invitee, Role::Admin(None), inviter, Utc::now());
    accounts
        .create_invitation(&reissued)
        .await
        .expect("re-invite after revoke");
    assert_eq!(
        accounts
            .find_pending_invitation(account.id, invitee)
            .await
            .expect("find_pending_invitation")
            .map(|i| i.id),
        Some(reissued.id),
        "re-inviting after a revoke seats a new pending offer"
    );
}

// An invitation id we never persisted resolves to nothing.
#[tokio::test]
async fn find_unknown_invitation_is_none() {
    let (pool, _container) = fresh_pool().await;
    let (accounts, account, inviter, invitee) = invitation_fixture(&pool, "ghost").await;

    // Issued in the domain but never persisted, so its id is genuinely unknown.
    let unstored = Invitation::issue(account.id, invitee, Role::Member(None), inviter, Utc::now());

    let found = accounts
        .find_invitation(unstored.id)
        .await
        .expect("find_invitation");
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
async fn seat_under(
    accounts: &PgAccountRepo,
    account: AccountId,
    invited: UserId,
    inviter: UserId,
) {
    let invitation = Invitation::issue(account, invited, Role::Member(None), inviter, Utc::now());
    accounts
        .create_invitation(&invitation)
        .await
        .expect("persist invitation");
    accounts
        .accept_invitation(invitation, true)
        .await
        .expect("accept seats the member under the inviter");
}

// AC3 — when a leaving member has children in the role tree, those children re-home
// to the leaver's Parent (DESIGN/Roles rule 3), and the leaver holds no role after.
#[tokio::test]
async fn leave_rehomes_children_to_the_leavers_parent() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:rehome-o".to_string()))
        .await
        .expect("owner");
    let a = users
        .provision(&Did::new("did:plc:rehome-a".to_string()))
        .await
        .expect("a");
    let b = users
        .provision(&Did::new("did:plc:rehome-b".to_string()))
        .await
        .expect("b");
    let c = users
        .provision(&Did::new("did:plc:rehome-c".to_string()))
        .await
        .expect("c");

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rehome-acct".to_string()),
        AccountName::try_new("Tree").unwrap(),
        Utc::now(),
    );
    accounts.create(&account, &membership).await.expect("found");
    seat_under(&accounts, account.id, a.id, owner.id).await; // A's parent is the Owner
    seat_under(&accounts, account.id, b.id, a.id).await; // B's parent is A
    seat_under(&accounts, account.id, c.id, a.id).await; // C's parent is A

    accounts.leave(a.id, account.id).await.expect("A leaves");

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
        accounts.role_of(a.id, account.id).await.expect("role_of"),
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
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let o1 = users
        .provision(&Did::new("did:plc:scope-o1".to_string()))
        .await
        .expect("o1");
    let o2 = users
        .provision(&Did::new("did:plc:scope-o2".to_string()))
        .await
        .expect("o2");
    let a = users
        .provision(&Did::new("did:plc:scope-a".to_string()))
        .await
        .expect("a");
    let b = users
        .provision(&Did::new("did:plc:scope-b".to_string()))
        .await
        .expect("b");
    let d = users
        .provision(&Did::new("did:plc:scope-d".to_string()))
        .await
        .expect("d");

    let (acct1, m1) = Account::open(
        o1.id,
        Did::new("did:plc:scope-acct1".to_string()),
        AccountName::try_new("One").unwrap(),
        Utc::now(),
    );
    accounts.create(&acct1, &m1).await.expect("found acct1");
    let (acct2, m2) = Account::open(
        o2.id,
        Did::new("did:plc:scope-acct2".to_string()),
        AccountName::try_new("Two").unwrap(),
        Utc::now(),
    );
    accounts.create(&acct2, &m2).await.expect("found acct2");

    // A parents B in acct1 and D in acct2.
    seat_under(&accounts, acct1.id, a.id, o1.id).await;
    seat_under(&accounts, acct1.id, b.id, a.id).await;
    seat_under(&accounts, acct2.id, a.id, o2.id).await;
    seat_under(&accounts, acct2.id, d.id, a.id).await;

    accounts
        .leave(a.id, acct1.id)
        .await
        .expect("A leaves acct1");

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
        accounts
            .role_of(a.id, acct2.id)
            .await
            .expect("role_of")
            .is_some(),
        "A is still a member of the account they didn't leave"
    );
}

// ZMVP-40 — leaving revokes the leaver's still-pending *issued* invitations (so none
// later seats a member under a non-member), and only those.
#[tokio::test]
async fn leave_revokes_the_leavers_pending_issued_invitations() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:rev-o".to_string()))
        .await
        .expect("owner");
    let a = users
        .provision(&Did::new("did:plc:rev-a".to_string()))
        .await
        .expect("a");
    let x = users
        .provision(&Did::new("did:plc:rev-x".to_string()))
        .await
        .expect("x");
    let y = users
        .provision(&Did::new("did:plc:rev-y".to_string()))
        .await
        .expect("y");

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:rev-acct".to_string()),
        AccountName::try_new("Studio").unwrap(),
        Utc::now(),
    );
    accounts.create(&account, &membership).await.expect("found");
    seat_under(&accounts, account.id, a.id, owner.id).await;

    // A (leaving) has a pending offer out to X; the Owner (staying) has one out to Y.
    let a_invites_x = Invitation::issue(account.id, x.id, Role::Member(None), a.id, Utc::now());
    accounts
        .create_invitation(&a_invites_x)
        .await
        .expect("A's invite");
    let owner_invites_y =
        Invitation::issue(account.id, y.id, Role::Member(None), owner.id, Utc::now());
    accounts
        .create_invitation(&owner_invites_y)
        .await
        .expect("Owner's invite");

    accounts.leave(a.id, account.id).await.expect("A leaves");

    let a_offer = accounts
        .find_invitation(a_invites_x.id)
        .await
        .expect("find")
        .expect("A's offer still exists as a record");
    assert_eq!(
        a_offer.state,
        InvitationState::Revoked,
        "the leaver's issued offer is revoked, not deleted"
    );
    assert!(
        accounts
            .find_pending_invitation(account.id, y.id)
            .await
            .expect("find pending")
            .is_some(),
        "an offer issued by someone still present stays pending"
    );
}
