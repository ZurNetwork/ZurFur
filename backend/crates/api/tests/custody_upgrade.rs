//! The custody upgrade end to end, on a test container through the served stack. A 0.1
//! database with two Accounts is refused under a wrong root, checked, then migrated and
//! re-wrapped under the right one; served, it changes a handle, deletes an Account (its
//! tombstone signed), founds a third and signs in and out; migrated again it changes
//! nothing; rolled back to the parent's schema, the 0.1 opener still opens both rows.

use adapter_pg::{
    CustodyCheck, CustodyMigrationError, CustodyMigrator, MigrationReport, PgDatabase,
    PgPlcOperationLog, PgPool,
};
use chrono::Utc;
use composition::Runtime;
use domain::elements::{
    account::{Account, AccountName},
    custody::Purpose,
    did::Did,
    handle::Handle,
    profile::Profile,
};
use domain::ports::{PlcOperationLog as _, UnitOfWork};
use serde_json::{Value, json};
use test_support::custody::{
    self, CUSTODY_ENVELOPE, LEGACY_ALICE_DID, LEGACY_BOB_DID, LEGACY_ROOT, root_of,
};
use test_support::pg_app::{self, PgServed};

/// The custody upgrade's second migration: it deletes the in-flight sign-in rows.
const SIGNIN_ROWS_DELETE: i64 = 20261022231357;

/// The first migration after the custody upgrade: the commission dispute columns, rolled back
/// before it.
const DISPUTE_COLUMNS: i64 = 20261026075811;

/// The second migration after the custody upgrade: the Slot node fill. The rehearsal forgets it
/// and leaves its nodes and checks in place, since the parent's schema reads neither.
const SLOT_FILL: i64 = 20261026080502;

/// The purposes the API boot derives.
const BOOT_PURPOSES: [Purpose; 3] = [Purpose::DidCustody, Purpose::SigninSecrets, Purpose::DenMac];

/// A root the 0.1 rows don't open under.
const WRONG_ROOT: [u8; 32] = [8u8; 32];

/// The User who owns both 0.1 Accounts and acts over HTTP.
const OWNER_DID: &str = "did:plc:legacyowner";

/// The Owner's handle on their PDS.
const OWNER_PDS_HANDLE: &str = "legacyowner.test";

/// The 0.1 Accounts: their DIDs (the fixtures' own), handles and names.
const LEGACY_ACCOUNTS: [(&str, &str, &str); 2] = [
    (LEGACY_ALICE_DID, "legacy-alice.zurfur.app", "Legacy Alice"),
    (LEGACY_BOB_DID, "legacy-bob.zurfur.app", "Legacy Bob"),
];

/// The handle Alice's Account moves to.
const RENAMED_HANDLE: &str = "upgraded-alice.zurfur.app";

/// The handle of the Account founded after the upgrade.
const THIRD_HANDLE: &str = "upgrade-third.zurfur.app";

/// What every DID-custody envelope's key id begins with.
const DID_CUSTODY_KEY_ID: &str = "hkdf:did_custody:";

/// What every sign-in row's key id begins with.
const SIGNIN_SECRETS_KEY_ID: &str = "hkdf:signin_secrets:";

/// The custody envelope's version byte.
const ENVELOPE_VERSION_BYTE: u8 = 0x01;

/// The Owner's profile on their PDS.
fn owner_profile() -> Profile {
    Profile::new(Did::from(OWNER_DID.to_owned()), OWNER_PDS_HANDLE)
}

/// The Owner, both 0.1 Accounts with their Owner memberships, the genesis operations their
/// foundings logged, and their 0.1 custody rows, stored on the schema before the envelope.
async fn seed_legacy_accounts(pool: &PgPool) {
    let owner_did = Did::from(OWNER_DID.to_owned());
    let founded_at = Utc::now();
    let database = PgDatabase::new(pool.clone());
    application::transaction(&database, async move |uow: &mut dyn UnitOfWork| {
        let owner = uow.users().provision(&owner_did).await?;
        for (did, handle, name) in LEGACY_ACCOUNTS {
            let (account, membership) = Account::open(
                owner.id.clone(),
                Did::from(did.to_owned()),
                handle.parse::<Handle>()?,
                name.parse::<AccountName>()?,
                founded_at,
            );
            uow.accounts().create(&account, &membership).await?;
        }
        Ok(())
    })
    .await
    .expect("the 0.1 Accounts are stored");

    let op_log = PgPlcOperationLog::new(pool.clone());
    for (did, handle, _) in LEGACY_ACCOUNTS {
        op_log
            .append(&custody::legacy_genesis_record(did, handle))
            .await
            .expect("the genesis operation is logged");
    }
    custody::seed_alice_and_bob(pool).await;
}

/// A pending sign-in and a stored OAuth session, as the parent build left them.
async fn seed_inflight_signin(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO atproto_oauth.client_session (account_did, session_id, data)
         VALUES ($1, 'legacy-session', $2)",
    )
    .bind(OWNER_DID)
    .bind(b"legacy session".as_slice())
    .execute(pool)
    .await
    .expect("seed a stored OAuth session");
    sqlx::query("INSERT INTO atproto_oauth.auth_request (state, data) VALUES ('legacy-state', $1)")
        .bind(b"legacy auth request".as_slice())
        .execute(pool)
        .await
        .expect("seed a pending sign-in");
}

/// Every custody row's DID and 0.1 ciphertext, by DID.
async fn wrapped_keys(pool: &PgPool) -> Vec<(String, Option<Vec<u8>>)> {
    sqlx::query_as("SELECT did, wrapped_keys FROM account_keys ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the 0.1 ciphertexts")
}

/// Every custody row's DID and sealed envelope, by DID.
async fn sealed_keys(pool: &PgPool) -> Vec<(String, Option<Vec<u8>>)> {
    sqlx::query_as("SELECT did, sealed_keys FROM account_keys ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the sealed envelopes")
}

/// Every stored sign-in row: its table, then its sealed data.
async fn signin_rows(pool: &PgPool) -> Vec<(String, Vec<u8>)> {
    sqlx::query_as(
        "SELECT 'client_session', data FROM atproto_oauth.client_session
         UNION ALL
         SELECT 'auth_request', data FROM atproto_oauth.auth_request",
    )
    .fetch_all(pool)
    .await
    .expect("read the sign-in rows")
}

/// The type and `prev` of each PLC operation logged for `did`, oldest first.
async fn plc_chain(pool: &PgPool, did: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as("SELECT type, prev FROM plc_operations WHERE did = $1 ORDER BY seq")
        .bind(did)
        .fetch_all(pool)
        .await
        .expect("read the PLC operation log")
}

/// The migration ledger's versions, ascending.
async fn ledger_versions(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read the ledger")
}

/// The custody migrator the API boot builds over `database_url` with `root`.
async fn boot_migrator(database_url: &str, root: [u8; 32]) -> CustodyMigrator {
    let config = pg_app::dev_config("http://localhost", database_url);
    let connecting = Runtime::connect_migrating(config, root_of(root), &BOOT_PURPOSES);
    let (_runtime, migrator) = connecting.await.expect("the migrating runtime connects");
    migrator
}

/// The embedded migration versions `keep` holds, ascending.
fn embedded_versions(keep: impl Fn(i64) -> bool) -> Vec<i64> {
    adapter_pg::migrator()
        .migrations
        .iter()
        .map(|migration| migration.version)
        .filter(|version| keep(*version))
        .collect()
}

/// The genesis operation's CID for `did`, as [`custody::legacy_genesis_record`] logs it.
fn genesis_cid(did: &str) -> String {
    custody::legacy_genesis_record(did, "unused.zurfur.app").cid
}

/// `PATCH /accounts/{did}/handle` to `handle` as `client`.
async fn change_handle(
    served: &PgServed,
    client: &reqwest::Client,
    did: &str,
    handle: &str,
) -> reqwest::Response {
    let change = json!({ "handle": handle });
    client
        .patch(format!("{}/accounts/{did}/handle", served.base_url))
        .json(&change)
        .send()
        .await
        .expect("PATCH /accounts/{id}/handle is answered")
}

/// The handles of the Accounts `client` belongs to, sorted.
async fn listed_handles(served: &PgServed, client: &reqwest::Client) -> Vec<String> {
    let response = client
        .get(format!("{}/accounts", served.base_url))
        .send()
        .await
        .expect("GET /accounts is answered");
    assert_eq!(response.status(), 200, "GET /accounts");
    let body: Value = response.json().await.expect("a JSON body");
    let mut handles: Vec<String> = body["accounts"]
        .as_array()
        .expect("an accounts list")
        .iter()
        .map(|account| account["handle"].as_str().expect("a handle").to_owned())
        .collect();
    handles.sort();
    handles
}

#[tokio::test]
async fn a_0_1_database_upgrades_serves_reruns_and_rolls_back() {
    // 1. A 0.1 database: two Accounts, their 0.1 custody rows, in-flight sign-in rows.
    let id_1_versions =
        embedded_versions(|version| (CUSTODY_ENVELOPE..=SIGNIN_ROWS_DELETE).contains(&version));
    assert_eq!(
        id_1_versions,
        [CUSTODY_ENVELOPE, SIGNIN_ROWS_DELETE],
        "the custody upgrade's two migrations"
    );
    let later_versions = embedded_versions(|version| version > SIGNIN_ROWS_DELETE);
    assert_eq!(
        later_versions,
        [DISPUTE_COLUMNS, SLOT_FILL],
        "the migrations after the custody upgrade"
    );
    let parent_versions = embedded_versions(|version| version < CUSTODY_ENVELOPE);
    let (pool, db) = custody::before_the_envelope().await;
    let database_url = db.url().to_owned();
    seed_legacy_accounts(&pool).await;
    seed_inflight_signin(&pool).await;
    let wrapped_before = wrapped_keys(&pool).await;
    let before = custody::store_snapshot(&pool).await;
    assert_eq!(ledger_versions(&pool).await, parent_versions);
    assert_eq!(wrapped_before.len(), 2, "two 0.1 custody rows");
    assert!(
        !before.columns.iter().any(|column| column == "sealed_keys"),
        "no envelope column before the upgrade: {:?}",
        before.columns
    );
    assert_eq!(
        signin_rows(&pool).await.len(),
        2,
        "two in-flight sign-in rows"
    );

    // 2. A wrong root: refused before any migration, by count.
    let refused = boot_migrator(&database_url, WRONG_ROOT).await.run().await;

    let refusal = refused.expect_err("a wrong root refuses the run");
    assert!(
        matches!(refusal, CustodyMigrationError::Unopenable { count: 2 }),
        "{refusal:?}"
    );
    assert_eq!(
        refusal.to_string(),
        "2 custody rows don't open under the configured root"
    );
    assert_eq!(
        custody::store_snapshot(&pool).await,
        before,
        "a refused run changes no ledger row, column or custody row"
    );

    // 3. The check under the right root: counts, writes nothing.
    let migrator = boot_migrator(&database_url, LEGACY_ROOT).await;
    let checked = migrator.check().await.expect("the check counts");

    let both_pending = CustodyCheck {
        unopenable: 0,
        pending: 2,
    };
    assert_eq!(checked, both_pending);
    assert_eq!(
        custody::store_snapshot(&pool).await,
        before,
        "the check wrote nothing"
    );

    // 4. The right root: the probe and dry run pass, the migrations run, both rows re-wrap.
    let report = migrator.run().await.expect("the upgrade runs");

    let latest = embedded_versions(|_| true).last().copied();
    let upgraded = MigrationReport {
        applied: id_1_versions.len() + later_versions.len(),
        version: latest,
        rows_rewrapped: 2,
    };
    assert_eq!(report, upgraded);
    assert_eq!(
        wrapped_keys(&pool).await,
        wrapped_before,
        "the 0.1 ciphertexts are byte-identical after the re-wrap"
    );
    let sealed = sealed_keys(&pool).await;
    assert_eq!(sealed.len(), 2, "{sealed:?}");
    for (did, envelope) in &sealed {
        let envelope = envelope
            .as_deref()
            .unwrap_or_else(|| panic!("{did} holds an envelope"));
        assert_eq!(envelope.first(), Some(&ENVELOPE_VERSION_BYTE), "{did}");
        let key_id = custody::envelope_key_id(envelope);
        assert!(
            key_id.starts_with(DID_CUSTODY_KEY_ID),
            "{did}'s envelope is sealed under the DID-custody key, not {key_id}"
        );
        assert_eq!(
            key_id,
            custody::key_id_of(&LEGACY_ROOT, Purpose::DidCustody)
        );
    }
    assert_eq!(
        signin_rows(&pool).await,
        Vec::new(),
        "both atproto_oauth tables are empty"
    );

    // 5. Over HTTP on the upgraded database.
    let served = pg_app::serve_over(db, api::app, root_of(LEGACY_ROOT)).await;
    let owner = served.sign_in(owner_profile()).await;

    let renamed = change_handle(&served, &owner, LEGACY_ALICE_DID, RENAMED_HANDLE).await;
    assert_eq!(renamed.status(), 200, "the handle change succeeds");
    let renamed_body: Value = renamed.json().await.expect("a JSON body");
    assert_eq!(renamed_body["handle"], RENAMED_HANDLE);
    let alice_chain = vec![
        ("plc_operation".to_owned(), None),
        (
            "plc_operation".to_owned(),
            Some(genesis_cid(LEGACY_ALICE_DID)),
        ),
    ];
    assert_eq!(
        plc_chain(&pool, LEGACY_ALICE_DID).await,
        alice_chain,
        "the handle update is signed with the re-wrapped keys"
    );

    let deleted = owner
        .delete(format!("{}/accounts/{LEGACY_BOB_DID}", served.base_url))
        .send()
        .await
        .expect("DELETE /accounts/{id} is answered");
    assert_eq!(deleted.status(), 200, "the delete succeeds");
    let deleted_body: Value = deleted.json().await.expect("a JSON body");
    assert_eq!(deleted_body["outcome"], "hard");
    let bob_chain = vec![
        ("plc_operation".to_owned(), None),
        (
            "plc_tombstone".to_owned(),
            Some(genesis_cid(LEGACY_BOB_DID)),
        ),
    ];
    assert_eq!(
        plc_chain(&pool, LEGACY_BOB_DID).await,
        bob_chain,
        "the tombstone is signed with the re-wrapped keys"
    );

    let founding = json!({ "name": "Upgrade Third", "handle": THIRD_HANDLE });
    let founded = owner
        .post(format!("{}/accounts", served.base_url))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts is answered");
    assert_eq!(founded.status(), 201, "the founding succeeds");
    let founded_body: Value = founded.json().await.expect("a JSON body");
    let third_did = founded_body["did"]
        .as_str()
        .expect("the founded Account's DID")
        .to_owned();
    let third_wrapped = wrapped_keys(&pool)
        .await
        .into_iter()
        .find(|(did, _)| *did == third_did)
        .expect("the third Account's custody row");
    assert_eq!(
        third_wrapped.1, None,
        "a new founding writes no 0.1 ciphertext"
    );
    let third_sealed = sealed_keys(&pool)
        .await
        .into_iter()
        .find_map(|(did, envelope)| (did == third_did).then_some(envelope))
        .flatten()
        .expect("the third Account's keys are sealed");
    assert!(custody::envelope_key_id(&third_sealed).starts_with(DID_CUSTODY_KEY_ID));

    let on_screen = vec![THIRD_HANDLE.to_owned(), RENAMED_HANDLE.to_owned()];
    assert_eq!(listed_handles(&served, &owner).await, on_screen);

    // The served sign-in answers through the in-process PDS; the auth store the live
    // sign-in writes through seals a session and a pending sign-in on this database.
    let signin_sealer = custody::custody_of(LEGACY_ROOT).signin_secrets;
    custody::signin_secrets_round_trip(served.runtime.pool.clone(), signin_sealer).await;
    let stored_signin = signin_rows(&pool).await;
    let mut stored_tables: Vec<&str> = stored_signin
        .iter()
        .map(|(table, _)| table.as_str())
        .collect();
    stored_tables.sort_unstable();
    assert_eq!(stored_tables, ["auth_request", "client_session"]);
    for (table, data) in &stored_signin {
        let key_id = custody::envelope_key_id(data);
        assert!(
            key_id.starts_with(SIGNIN_SECRETS_KEY_ID),
            "a {table} row is sealed under the sign-in key, not {key_id}"
        );
    }

    let signed_out = owner
        .post(format!("{}/logout", served.base_url))
        .send()
        .await
        .expect("POST /logout is answered");
    assert_eq!(signed_out.status(), 303, "sign-out redirects home");
    let after_sign_out = owner
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me is answered");
    assert_eq!(after_sign_out.status(), 401, "the session is gone");

    // 6. Run again: nothing applies, nothing re-wraps, nothing changes.
    let before_rerun = custody::store_snapshot(&pool).await;

    let rerun = boot_migrator(&database_url, LEGACY_ROOT)
        .await
        .run()
        .await
        .expect("the rerun runs");

    let nothing_to_do = MigrationReport {
        applied: 0,
        version: latest,
        rows_rewrapped: 0,
    };
    assert_eq!(rerun, nothing_to_do);
    assert_eq!(custody::store_snapshot(&pool).await, before_rerun);

    // 7. Rollback rehearsal: the dispute columns rolled back first, then the rollback recipe's
    // statements, in its order and in one transaction, back to the parent's schema, where the
    // 0.1 opener opens both.
    let mut rollback = pool.begin().await.expect("the rollback begins");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ANY($1)")
        .bind(&later_versions)
        .execute(&mut *rollback)
        .await
        .expect("forget the migrations after the upgrade");
    sqlx::query(
        "ALTER TABLE commission
             DROP CONSTRAINT commission_dispute_matches_its_step,
             DROP COLUMN disputed_from,
             DROP COLUMN disputed_at",
    )
    .execute(&mut *rollback)
    .await
    .expect("roll back the dispute columns first");
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ANY($1)")
        .bind(&id_1_versions)
        .execute(&mut *rollback)
        .await
        .expect("forget the upgrade's migrations");
    sqlx::query("DELETE FROM account_keys WHERE wrapped_keys IS NULL")
        .execute(&mut *rollback)
        .await
        .expect("drop every envelope-only custody row");
    sqlx::query(
        "ALTER TABLE account_keys
             DROP CONSTRAINT account_keys_holds_keys,
             DROP COLUMN sealed_keys,
             ALTER COLUMN wrapped_keys SET NOT NULL",
    )
    .execute(&mut *rollback)
    .await
    .expect("restore the parent's custody table");
    sqlx::query("TRUNCATE atproto_oauth.client_session, atproto_oauth.auth_request")
        .execute(&mut *rollback)
        .await
        .expect("drop the sign-in rows sealed in the envelope");
    rollback.commit().await.expect("the rollback commits");

    let rolled_back = boot_migrator(&database_url, LEGACY_ROOT)
        .await
        .check()
        .await
        .expect("the check counts on the parent's schema");

    assert_eq!(ledger_versions(&pool).await, parent_versions);
    let after_rollback = custody::store_snapshot(&pool).await;
    assert_eq!(after_rollback.columns, before.columns);
    assert_eq!(wrapped_keys(&pool).await, wrapped_before);
    assert_eq!(
        signin_rows(&pool).await,
        Vec::new(),
        "no sign-in row sealed in the envelope is left"
    );
    assert_eq!(
        rolled_back, both_pending,
        "the 0.1 opener opens both rows on the rolled-back schema"
    );
}
