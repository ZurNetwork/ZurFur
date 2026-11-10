//! A 0.1 store on a test database: the schema just before the custody envelope, its
//! custody rows and the genesis operations its foundings logged, and a snapshot of what a
//! custody migration may change.

use adapter_pg::PgPool;
use atrium_crypto::keypair::{Did as _, Secp256k1Keypair};
use domain::elements::did::Did;
use domain::elements::plc_operation::PlcOperationRecord;

use super::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_DID, LEGACY_BOB_BLOB_HEX, LEGACY_BOB_DID, hex_bytes,
    legacy_keys,
};
use crate::pg::{TestDb, bare_db};

/// The custody envelope's migration, as sqlx numbers it: the custody upgrade's first.
pub const CUSTODY_ENVELOPE: i64 = 20261022231354;

/// The signature a [`legacy_genesis_record`] carries in place of a real one.
const STAND_IN_SIGNATURE: &str = "legacy-genesis-signature";

/// A bare test database migrated to just before [`CUSTODY_ENVELOPE`], and a pool on it.
pub async fn before_the_envelope() -> (PgPool, TestDb) {
    let db = bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    let mut embedded = adapter_pg::migrator();
    let before_the_envelope: Vec<_> = embedded
        .migrations
        .iter()
        .filter(|migration| migration.version < CUSTODY_ENVELOPE)
        .cloned()
        .collect();
    assert!(
        before_the_envelope.len() < embedded.migrations.len(),
        "the envelope's version names an embedded migration"
    );
    embedded.migrations = before_the_envelope.into();
    embedded
        .run(&pool)
        .await
        .expect("the migrations before the envelope run");
    (pool, db)
}

/// Stores one 0.1 custody row, naming only the columns the table had before the envelope.
pub async fn seed_legacy(pool: &PgPool, did: &str, wrapped_keys: &[u8]) {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, created_at)
         VALUES ($1, $2, now())",
    )
    .bind(did)
    .bind(wrapped_keys)
    .execute(pool)
    .await
    .expect("seed a 0.1 row");
}

/// Stores Alice's and Bob's 0.1 custody rows.
pub async fn seed_alice_and_bob(pool: &PgPool) {
    seed_legacy(pool, LEGACY_ALICE_DID, &hex_bytes(LEGACY_ALICE_BLOB_HEX)).await;
    seed_legacy(pool, LEGACY_BOB_DID, &hex_bytes(LEGACY_BOB_BLOB_HEX)).await;
}

/// The identity-only genesis operation a 0.1 founding logged for `did` under `handle`: its
/// rotation and signing keys are the did:keys of [`legacy_keys`]. Its CID and signature are
/// stand-ins, since nothing local checks them.
pub fn legacy_genesis_record(did: &str, handle: &str) -> PlcOperationRecord {
    let keys = legacy_keys();
    let did_key_of = |secret: &[u8]| {
        Secp256k1Keypair::import(secret)
            .expect("a legacy key is a secp256k1 scalar")
            .did()
    };
    let operation = serde_json::json!({
        "type": "plc_operation",
        "rotationKeys": [
            did_key_of(keys.cold_recovery.expose()),
            did_key_of(keys.operational.expose()),
        ],
        "verificationMethods": { "atproto": did_key_of(keys.signing.expose()) },
        "alsoKnownAs": [format!("at://{handle}")],
        "services": {},
        "prev": null,
        "sig": STAND_IN_SIGNATURE,
    });
    PlcOperationRecord {
        did: Did::from(did.to_owned()),
        cid: format!("legacy-genesis-{did}"),
        op_type: "plc_operation".to_owned(),
        prev: None,
        operation_json: operation.to_string(),
    }
}

/// Everything a custody migration may change, whole, as text: the migration ledger's rows,
/// the custody table's columns and its rows.
#[derive(Debug, PartialEq, Eq)]
pub struct StoreSnapshot {
    pub ledger: Vec<String>,
    pub columns: Vec<String>,
    pub rows: Vec<String>,
}

/// The [`StoreSnapshot`] of `pool`'s database.
pub async fn store_snapshot(pool: &PgPool) -> StoreSnapshot {
    let ledger =
        sqlx::query_scalar("SELECT to_jsonb(m)::text FROM _sqlx_migrations m ORDER BY version")
            .fetch_all(pool)
            .await
            .expect("read the ledger");
    let columns = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = 'account_keys'
         ORDER BY ordinal_position",
    )
    .fetch_all(pool)
    .await
    .expect("read the custody columns");
    let rows = sqlx::query_scalar("SELECT to_jsonb(k)::text FROM account_keys k ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the custody rows");
    StoreSnapshot {
        ledger,
        columns,
        rows,
    }
}
