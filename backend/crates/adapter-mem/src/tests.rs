use super::*;

fn did(s: &str) -> Did {
    Did::from(s.to_string())
}

// Criterion 2 — "one DID, one User, forever". A repeat sign-in must find the
// very same User: same id, same created_at. If the second call minted afresh,
// either would differ (the id is a new UUIDv7, created_at a later instant).
#[tokio::test]
async fn provision_is_idempotent_per_did() {
    let backend = MemBackend::new();
    let d = did("did:plc:alice");

    let first = backend.provision(&d).await.unwrap();
    let second = backend.provision(&d).await.unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(first.created_at, second.created_at);
    assert_eq!(second.id.did(), &d);
}

// Distinct DIDs are distinct Users — recognition is keyed by DID, never shared.
#[tokio::test]
async fn distinct_dids_get_distinct_users() {
    let backend = MemBackend::new();

    let alice = backend.provision(&did("did:plc:alice")).await.unwrap();
    let bob = backend.provision(&did("did:plc:bob")).await.unwrap();

    assert_ne!(alice.id, bob.id);
}

// Criterion 3 — a session resolves back to its User by id, no PDS round-trip.
#[tokio::test]
async fn find_returns_the_provisioned_user() {
    let backend = MemBackend::new();
    let provisioned = backend.provision(&did("did:plc:alice")).await.unwrap();

    let found = backend.user_store().find(&provisioned.id).await.unwrap();

    assert_eq!(found, Some(provisioned));
}

// An id we never minted resolves to nothing — an expired or forged session id
// greets no one.
#[tokio::test]
async fn find_unknown_id_returns_none() {
    let backend = MemBackend::new();
    backend.provision(&did("did:plc:alice")).await.unwrap();

    let found = backend.user_store().find(&user_id()).await.unwrap();

    assert_eq!(found, None);
}

// ZMVP-123 — the mem two-step create: provisioning interns the actor_identity
// parent alongside the user (findable by the DID both are keyed by since the
// actor re-key, DD 57081857), and a dropped unit discards BOTH, mirroring pg's
// rollback of the projection and its parent together.
#[tokio::test]
async fn provision_interns_the_identity_and_rolls_back_together() {
    let backend = MemBackend::new();
    let database = backend.database();
    let identities = backend.actor_identity_store();

    // Committed provision: the user and its identity both land, sharing the id.
    let d = did("did:plc:mem-two-step");
    let user = backend.provision(&d).await.unwrap();
    let identity = identities
        .find_by_did(&d)
        .await
        .unwrap()
        .expect("the identity was interned alongside the user");
    assert_eq!(
        identity.did.as_ref(),
        Some(user.id.did()),
        "the user and its identity are keyed by the same DID"
    );
    assert_eq!(identity.kind, ActorKind::User);

    // A dropped unit discards both the user projection and its interned identity.
    let rolled = did("did:plc:mem-rolled-back");
    {
        let mut uow = database.begin().await.unwrap();
        uow.users().provision(&rolled).await.unwrap();
        // Dropped without commit → rollback.
    }
    assert!(
        backend.find_by_did(&rolled).await.unwrap().is_none(),
        "no user survives a dropped unit"
    );
    assert!(
        identities.find_by_did(&rolled).await.unwrap().is_none(),
        "no interned identity survives either"
    );
}

/// A fresh, unique synthetic actor DID; the UUID is only a uniqueness source.
fn mint_did() -> Did {
    Did::from(format!("did:plc:mem{}", uuid::Uuid::now_v7().simple()))
}

fn user_id() -> UserId {
    UserId::from(mint_did())
}

// Builds a live account directly. Repo tests exercise storage, so they don't go
// through the founding invariant (`Account::open`, ZMVP-14 #1) — that path is
// covered end-to-end by the api `accounts.rs` test, which drives `POST /accounts`.
fn live_account(did_s: &str) -> Account {
    let now = Utc::now();
    // Derive a valid, distinct handle from the did so accounts built for
    // different dids never collide on the unique handle.
    let label: String = did_s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    Account {
        id: AccountId::from(did(did_s)),
        handle: format!("{label}.example.com").parse::<Handle>().unwrap(),
        name: "Test Studio".parse::<AccountName>().unwrap(),
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

// The mem seam, end to end: a write issued through the UnitOfWork's account view
// (begin → accounts().create → commit) is visible to a later read off the shared
// AccountStore — proving the read store, the factory, and the write view share
// state. Founding persists the account; `find` reads it back by id.
#[tokio::test]
async fn uow_create_is_visible_to_the_read_store() {
    let backend = MemBackend::new();
    let database = backend.database();
    let accounts = backend.account_store();

    let account = live_account("did:plc:acct");
    let (id, account_name) = (account.id.clone(), account.name.clone());
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };

    let mut uow = database.begin().await.unwrap();
    uow.accounts().create(&account, &owner).await.unwrap();
    uow.commit().await.unwrap();

    let found = accounts.find(&id).await.unwrap().expect("account present");
    assert_eq!(found.id, id);
    assert_eq!(found.name, account_name); // the name round-trips
    assert_eq!(found.deleted_at, None);
}

// Dropping a unit of work before `commit()` discards EVERY write in it — the mem
// mirror of pg's drop = rollback (DD 24150017, mirroring the pg
// `a_dropped_unit_of_work_rolls_back_every_write`). `create` stages two writes (the
// account + the owner membership); once the handle drops uncommitted, neither
// reaches the shared read store. This is what makes the rollback fidelity real and
// exercised — a forgotten `commit()` now leaves nothing behind in mem too, so a mem
// test can catch it.
#[tokio::test]
async fn a_dropped_unit_of_work_rolls_back_every_write() {
    let backend = MemBackend::new();
    let database = backend.database();
    let accounts = backend.account_store();

    let account = live_account("did:plc:rollback");
    let account_id = account.id.clone();
    let owner_id = user_id();
    let owner = UserAccount {
        user_id: owner_id.clone(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };

    // Open the unit, stage the (two-write) create, then drop WITHOUT committing.
    {
        let mut uow = database.begin().await.unwrap();
        uow.accounts().create(&account, &owner).await.unwrap();
        // `uow` drops here without `commit` → the staged writes are discarded.
    }

    assert!(
        accounts.find(&account_id).await.unwrap().is_none(),
        "a dropped unit of work persists no account row"
    );
    assert_eq!(
        accounts.role_of(&owner_id, &account_id).await.unwrap(),
        None,
        "...and no membership either — both staged writes rolled back together"
    );
}

// An uncommitted unit's writes are invisible to a concurrent read off the shared
// store *before* the unit commits — matching pg, where a pool read can't see
// another connection's open transaction. Here the read store sees nothing until
// `commit`, then sees the account.
#[tokio::test]
async fn uncommitted_writes_are_invisible_until_commit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let accounts = backend.account_store();

    let account = live_account("did:plc:isolated");
    let account_id = account.id.clone();
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };

    let mut uow = database.begin().await.unwrap();
    uow.accounts().create(&account, &owner).await.unwrap();
    // Still open, not committed: the shared read store must not see it yet.
    assert!(
        accounts.find(&account_id).await.unwrap().is_none(),
        "an open unit's staged write is invisible to a shared read"
    );

    uow.commit().await.unwrap();
    assert!(
        accounts.find(&account_id).await.unwrap().is_some(),
        "the write becomes visible once the unit commits"
    );
}

// The founder's Owner membership is minted alongside the account — `role_of`
// returns it for the (user, account) pair, read off the shared store.
#[tokio::test]
async fn role_of_owner_returns_owner() {
    let backend = MemBackend::new();
    let account = live_account("did:plc:acct");
    let owner_id = user_id();
    let owner = UserAccount {
        user_id: owner_id.clone(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };
    let account_id = account.id.clone();

    backend.create(&account, &owner).await.unwrap();

    let role = backend.role_of(&owner_id, &account_id).await.unwrap();
    assert_eq!(role, Some(Role::Owner));
}

// An account we never founded resolves to nothing.
#[tokio::test]
async fn find_unknown_account_returns_none() {
    let backend = MemBackend::new();
    let account = live_account("did:plc:acct");
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account.id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    let other = live_account("did:plc:other");
    let found = backend.find(&other.id).await.unwrap();

    assert_eq!(found.map(|a| a.id), None);
}

// ZMVP-46 — the change flow's private half through the UnitOfWork: `change_handle`
// repoints resolution (new resolves, old doesn't), and records the change so the
// rate-limit count sees it. Staged like any account write, visible after commit.
#[tokio::test]
async fn change_handle_repoints_resolution_and_is_counted() {
    let backend = MemBackend::new();
    let database = backend.database();
    let store = backend.account_store();

    let account = live_account("did:plc:memchg");
    let (old, account_id) = (account.handle.clone(), account.id.clone());
    let account_did = account_id.did().clone();
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account_id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    let new = "memchg-new.example.com".parse::<Handle>().unwrap();
    let mut uow = database.begin().await.unwrap();
    uow.accounts()
        .change_handle(&account_id, &old, &new, Utc::now())
        .await
        .unwrap();
    uow.commit().await.unwrap();

    assert_eq!(
        store.find_did_by_handle(&new).await.unwrap(),
        Some(account_did),
        "the new handle resolves to the account's DID"
    );
    assert!(
        store.find_did_by_handle(&old).await.unwrap().is_none(),
        "the old handle no longer resolves"
    );
    assert_eq!(
        store
            .count_handle_changes_since(&account_id, Utc::now() - chrono::Duration::minutes(5))
            .await
            .unwrap(),
        1,
        "the change is counted for the rate limit"
    );
}

// ZMVP-46 §4 — the vacated handle is quarantined to the leaving account: barred to
// another account, excluded (reclaimable) for the account that left it.
#[tokio::test]
async fn change_handle_quarantines_the_vacated_handle() {
    let backend = MemBackend::new();
    let database = backend.database();
    let store = backend.account_store();

    let account = live_account("did:plc:memquar");
    let (old, account_id) = (account.handle.clone(), account.id.clone());
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account_id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    let new = "memquar-new.example.com".parse::<Handle>().unwrap();
    let mut uow = database.begin().await.unwrap();
    uow.accounts()
        .change_handle(&account_id, &old, &new, Utc::now())
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let window = Utc::now() - chrono::Duration::days(30);
    let stranger = live_account("did:plc:memquar-stranger").id;
    assert!(
        store
            .handle_reserved_for_other(&old, Some(&stranger), window)
            .await
            .unwrap(),
        "the vacated handle is reserved against another account"
    );
    assert!(
        !store
            .handle_reserved_for_other(&old, Some(&account_id), window)
            .await
            .unwrap(),
        "the leaving account may reclaim its own vacated handle"
    );
}

// Each mint yields a distinct DID — accounts never share a sovereign identity.
#[tokio::test]
async fn mint_returns_distinct_dids() {
    let minter = MemDidMinter::new();
    let handle = "alice.zurfur.app".parse::<Handle>().unwrap();

    let first = minter.mint(&handle).await.unwrap();
    let second = minter.mint(&handle).await.unwrap();

    assert_ne!(first, second);
}

// Parity with the real minter's port surface: the fake's update_handle is a
// no-op that never fails (it registers no real operation), so API-level
// handle-change tests run against mem without infrastructure.
#[tokio::test]
async fn mem_update_handle_is_a_noop() {
    let minter = MemDidMinter::new();
    let handle = "alice.zurfur.app".parse::<Handle>().unwrap();

    let did = minter.mint(&handle).await.unwrap();
    minter
        .update_handle(&did, &"bob.zurfur.app".parse::<Handle>().unwrap())
        .await
        .unwrap();
}

// The mem KeyStore round-trips custody keys: what you put is what you get.
#[tokio::test]
async fn mem_key_store_round_trips() {
    let store = MemKeyStore::new();
    let d = did("did:plc:alice");
    let keys = AccountKeys {
        cold_recovery: domain::elements::account_keys::SecretKey::new(vec![1u8; 32]),
        operational: domain::elements::account_keys::SecretKey::new(vec![2u8; 32]),
        signing: domain::elements::account_keys::SecretKey::new(vec![3u8; 32]),
    };

    assert!(store.get(&d).await.unwrap().is_none());
    store.put(&d, &keys).await.unwrap();
    assert_eq!(store.get(&d).await.unwrap().unwrap(), keys);
}

fn account_id() -> AccountId {
    AccountId::from(mint_did())
}

// AC3 (store layer) — a freshly issued pending invitation round-trips: it is the
// pending offer found for its (account, invited_user) pair, with every fact intact.
#[tokio::test]
async fn create_then_find_pending_returns_the_invitation() {
    let backend = MemBackend::new();
    let (account, invited, inviter) = (account_id(), user_id(), user_id());
    let invitation = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Admin,
        inviter.clone(),
        Utc::now(),
    );
    let id = invitation.id;

    backend.create_invitation(&invitation).await.unwrap();

    let found = backend
        .account_store()
        .find_pending_invitation(&account, &invited)
        .await
        .unwrap()
        .expect("the pending invitation is found");
    assert_eq!(found.id, id);
    assert_eq!(found.role, Role::Admin);
    assert_eq!(found.inviter, inviter);
    assert_eq!(found.state, InvitationState::Pending);
}

// AC5 (store layer) — at most one pending per (account, user): a second issue for
// the same pair while one is pending creates no second row.
#[tokio::test]
async fn a_second_pending_invitation_for_the_same_pair_is_not_a_second_row() {
    let backend = MemBackend::new();
    let (account, invited) = (account_id(), user_id());
    let first = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Member,
        user_id(),
        Utc::now(),
    );
    let second = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Admin,
        user_id(),
        Utc::now(),
    );

    backend.create_invitation(&first).await.unwrap();
    let standing = backend.create_invitation(&second).await.unwrap();

    // The dropped duplicate hands back the offer that actually stands, not the
    // one it proposed — the caller is never told its no-op took effect.
    assert_eq!(
        (standing.id, standing.role.clone()),
        (first.id, Role::Member),
        "the dropped duplicate returns the pending offer already on file"
    );

    // The original survives; the duplicate was a no-op (not a second row).
    let store = backend.account_store();
    let found = store
        .find_pending_invitation(&account, &invited)
        .await
        .unwrap()
        .expect("a pending invitation remains");
    assert_eq!(
        found.id, first.id,
        "the first pending offer is the one kept"
    );
    assert!(
        store.find_invitation(&second.id).await.unwrap().is_none(),
        "the duplicate issue stored nothing"
    );
}

// AC4 (store layer) — revoking flips the offer to revoked: it is no longer the
// pending offer for its pair, and a re-issue may now seat a fresh one. The revoke
// goes through the UnitOfWork write view; the reads off the shared store.
#[tokio::test]
async fn revoke_invitation_flips_state_and_clears_the_pending_offer() {
    let backend = MemBackend::new();
    let database = backend.database();
    let store = backend.account_store();
    let (account, invited) = (account_id(), user_id());
    let invitation = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Member,
        user_id(),
        Utc::now(),
    );
    let id = invitation.id;
    backend.create_invitation(&invitation).await.unwrap();

    let mut uow = database.begin().await.unwrap();
    uow.accounts().revoke_invitation(&id).await.unwrap();
    uow.commit().await.unwrap();

    assert_eq!(
        store.find_invitation(&id).await.unwrap().map(|i| i.state),
        Some(InvitationState::Revoked),
        "the invitation reads back revoked"
    );
    assert!(
        store
            .find_pending_invitation(&account, &invited)
            .await
            .unwrap()
            .is_none(),
        "a revoked invitation is no longer a live pending offer"
    );

    // With the prior offer revoked, a fresh invitation to the same pair is seated.
    let reissued = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Admin,
        user_id(),
        Utc::now(),
    );
    backend.create_invitation(&reissued).await.unwrap();
    assert_eq!(
        store
            .find_pending_invitation(&account, &invited)
            .await
            .unwrap()
            .map(|i| i.id),
        Some(reissued.id),
        "re-inviting after a revoke seats a new pending offer"
    );
}

// An invitation id we never stored resolves to nothing.
#[tokio::test]
async fn find_unknown_invitation_returns_none() {
    let backend = MemBackend::new();
    let found = backend
        .account_store()
        .find_invitation(&InvitationId::from(uuid::Uuid::now_v7()))
        .await
        .unwrap();
    assert!(found.is_none());
}

// Bug guard — mirrors pg's `ON CONFLICT (account_id, user_id) DO NOTHING`: a
// pending invitation accepted for a pair that's ALREADY seated (granted a role
// through another path while the offer sat pending) must not overwrite the
// existing membership. It's a no-op that keeps the ORIGINAL role, not the
// invitation's offer.
#[tokio::test]
async fn accepting_an_invitation_for_an_already_seated_pair_keeps_the_original_role() {
    let backend = MemBackend::new();
    let (account, invitee, inviter) = (account_id(), user_id(), user_id());

    // The invitee is granted Admin directly, bypassing any invitation.
    backend
        .grant_role(&UserAccount {
            account_id: account.clone(),
            user_id: invitee.clone(),
            role: Role::Admin,
            alias: None,
        })
        .await
        .unwrap();

    // A stale pending invitation (issued before the grant) offers only Member.
    let invitation = Invitation::issue(
        account.clone(),
        invitee.clone(),
        Role::Member,
        inviter,
        Utc::now(),
    );
    backend.create_invitation(&invitation).await.unwrap();

    let database = backend.database();
    let mut uow = database.begin().await.unwrap();
    let seated = uow
        .accounts()
        .accept_invitation(invitation, false)
        .await
        .expect("accepting an already-seated pair must not error");
    uow.commit().await.unwrap();

    assert_eq!(
        seated.role,
        Role::Admin,
        "the returned membership reflects the original grant, not the invitation's role"
    );
    assert_eq!(
        backend.role_of(&invitee, &account).await.unwrap(),
        Some(Role::Admin),
        "the persisted membership still holds the original grant"
    );
}

// Bug guard: hard_delete must drop the handle-change rows too, else a
// hard-deleted account keeps quarantining a handle nobody owns.
#[tokio::test]
async fn hard_delete_clears_the_accounts_handle_change_log() {
    let backend = MemBackend::new();
    let database = backend.database();
    let store = backend.account_store();

    let account = live_account("did:plc:hdchangelog");
    let (old, account_id) = (account.handle.clone(), account.id.clone());
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account_id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    let new = "hdchangelog-new.example.com".parse::<Handle>().unwrap();
    let mut uow = database.begin().await.unwrap();
    uow.accounts()
        .change_handle(&account_id, &old, &new, Utc::now())
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let since = Utc::now() - chrono::Duration::minutes(5);
    assert_eq!(
        store
            .count_handle_changes_since(&account_id, since)
            .await
            .unwrap(),
        1,
        "the change is recorded before the delete"
    );

    let mut uow = database.begin().await.unwrap();
    uow.accounts().hard_delete(&account_id).await.unwrap();
    uow.commit().await.unwrap();

    assert_eq!(
        store
            .count_handle_changes_since(&account_id, since)
            .await
            .unwrap(),
        0,
        "hard_delete drops the account's handle-change log rows too"
    );
}

// Bug guard: two units opened from the same shared state, each provisioning
// a DIFFERENT user, must both survive once committed.
#[tokio::test]
async fn two_units_each_provisioning_a_different_user_both_survive_commit() {
    let backend = MemBackend::new();
    let database = backend.database();

    // Both snapshot the same shared state before either has written anything.
    let mut first_unit = database.begin().await.unwrap();
    let mut second_unit = database.begin().await.unwrap();

    let alice = first_unit
        .users()
        .provision(&did("did:plc:merge-alice"))
        .await
        .unwrap();
    let bob = second_unit
        .users()
        .provision(&did("did:plc:merge-bob"))
        .await
        .unwrap();

    first_unit.commit().await.unwrap();
    second_unit.commit().await.unwrap();

    assert_eq!(
        backend
            .find_by_did(&did("did:plc:merge-alice"))
            .await
            .unwrap()
            .map(|u| u.id),
        Some(alice.id),
        "the first unit's disjoint write survives the second unit's commit"
    );
    assert_eq!(
        backend
            .find_by_did(&did("did:plc:merge-bob"))
            .await
            .unwrap()
            .map(|u| u.id),
        Some(bob.id),
        "the second unit's write is present too, not clobbered by the first"
    );
}

// Bug guard: a unit that merely rode along with a key in its snapshot must
// not clobber a concurrent unit's update of it back to the stale value.
#[tokio::test]
async fn a_third_units_untouched_read_does_not_clobber_a_concurrent_update() {
    let backend = MemBackend::new();
    let database = backend.database();

    let account = live_account("did:plc:merge-update");
    let account_id = account.id.clone();
    let old_handle = account.handle.clone();
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account_id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    // `unit_a` snapshots the account BEFORE `unit_b` updates it, but never
    // writes the account itself — only a disjoint key, committed after.
    let mut unit_a = database.begin().await.unwrap();
    let mut unit_b = database.begin().await.unwrap();

    let new_handle = "merge-update-new.example.com".parse::<Handle>().unwrap();
    unit_b
        .accounts()
        .change_handle(&account_id, &old_handle, &new_handle, Utc::now())
        .await
        .unwrap();
    unit_b.commit().await.unwrap();

    let other_account = live_account("did:plc:merge-update-other");
    let other_owner = UserAccount {
        user_id: user_id(),
        account_id: other_account.id.clone(),
        role: Role::Owner,
        alias: None,
    };
    unit_a
        .accounts()
        .create(&other_account, &other_owner)
        .await
        .unwrap();
    unit_a.commit().await.unwrap();

    let found_account = backend
        .find(&account_id)
        .await
        .unwrap()
        .expect("account still present");
    assert_eq!(
        found_account.handle, new_handle,
        "unit_b's committed update survives unit_a's later, disjoint commit"
    );
    assert!(
        backend.find(&other_account.id).await.unwrap().is_some(),
        "unit_a's own disjoint write is present too"
    );
}

// Bug guard: the stale-snapshot clobber also resurrected a key another unit
// had deleted.
#[tokio::test]
async fn a_third_units_untouched_read_does_not_resurrect_a_concurrent_delete() {
    let backend = MemBackend::new();
    let database = backend.database();

    let account = live_account("did:plc:merge-delete");
    let account_id = account.id.clone();
    let owner = UserAccount {
        user_id: user_id(),
        account_id: account_id.clone(),
        role: Role::Owner,
        alias: None,
    };
    backend.create(&account, &owner).await.unwrap();

    // `unit_a` snapshots the account BEFORE `unit_b` deletes it, but never
    // touches the account itself — only a disjoint key, committed after.
    let mut unit_a = database.begin().await.unwrap();
    let mut unit_b = database.begin().await.unwrap();

    unit_b.accounts().hard_delete(&account_id).await.unwrap();
    unit_b.commit().await.unwrap();

    let other_account = live_account("did:plc:merge-delete-other");
    let other_owner = UserAccount {
        user_id: user_id(),
        account_id: other_account.id.clone(),
        role: Role::Owner,
        alias: None,
    };
    unit_a
        .accounts()
        .create(&other_account, &other_owner)
        .await
        .unwrap();
    unit_a.commit().await.unwrap();

    assert!(
        backend.find(&account_id).await.unwrap().is_none(),
        "unit_b's committed delete stays deleted after unit_a's later, disjoint commit"
    );
    assert!(
        backend.find(&other_account.id).await.unwrap().is_some(),
        "unit_a's own disjoint write is present too"
    );
}

// The commission store-layer tests (ZMVP-65/87) live with the commission
// fakes in `crate::commission`.
