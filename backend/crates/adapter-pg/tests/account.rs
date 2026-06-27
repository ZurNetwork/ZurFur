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
        account::{Account, AccountName},
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
