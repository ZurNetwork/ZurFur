//! The custody canary: every custody path on PostgreSQL, under the dev subscriber at `trace`
//! with span fields, writes no root, legacy root, PRK or purpose key (as hex, base64 or a byte
//! list), no plaintext, envelope or MAC tag, no unchecked key-id text, no sign-in secret, and
//! no key id or DID at info or above. The paths: the guarded derivation, an Account's keys put
//! and got, an envelope with a forged key id opened, the custody migrator's check and run over
//! the 0.1 rows and its refusal under a wrong root, a sign-in session and an auth request
//! sealed and opened, and a Den MAC tagged, verified and refused. Alone in its binary: it
//! installs the global subscriber.

use adapter_pg::{CustodyCheck, CustodyMigrationError, PgKeyStore, PgPool};
use api::Environment;
use composition::{Config, Custody, Runtime};
use domain::elements::account_keys::{AccountKeys, SecretKey};
use domain::elements::custody::{
    CustodyError, CustodySubject, ENVELOPE_VERSION, MacMessage, MacUse, Purpose, SealedEnvelope,
};
use domain::elements::did::Did;
use domain::ports::KeyStore as _;
use strum::VariantArray as _;
use test_support::custody::{
    self, LEGACY_ALICE_DID, LEGACY_BOB_DID, LEGACY_ROOT, SIGNIN_CANARY_DID, SIGNIN_CANARY_SECRETS,
    legacy_keys, root_of,
};
use test_support::log_capture::CapturedLog;
use test_support::pg::TestDb;
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt as _};

/// The planted custody root, 32 bytes of text so its text form is searched too.
const PLANTED_ROOT: [u8; 32] = *b"custody-trace-planted-root-c4e1a";

/// The DID whose keys are put and got: a log may print it at debug only.
const KEYS_DID: &str = "did:plc:canarycustodytrace5f";

/// The DID whose stored envelope names a forged key id.
const FORGED_DID: &str = "did:plc:canaryforgedenvelope2b";

/// The forged key id: outside the key-id alphabet, a newline and an escape inside it.
const FORGED_KEY_ID: &[u8] = b"hkdf:did_custody:canarybadkeyid\n\x1b[31mforgedtail";

/// The pieces of [`FORGED_KEY_ID`] a log would show however it escapes the rest.
const FORGED_PIECES: [&str; 2] = ["canarybadkeyid", "forgedtail"];

/// The Den record the MAC is computed over.
const DEN_RECORD: &[u8] = b"canary-den-record-8d3e";

/// The table every DID-custody subject names first.
const KEYS_TABLE: &[u8] = b"account_keys";

/// The length of an envelope's nonce, and of the AEAD tag that ends it, in bytes.
const NONCE_BYTES: usize = 24;
const AEAD_TAG_BYTES: usize = 16;

/// The level of a formatted line: the first of its words that names one.
fn level_of(line: &str) -> Option<&str> {
    line.split_whitespace()
        .find(|word| ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].contains(word))
}

/// `text` without its ANSI color escapes.
fn without_colors(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_escape = false;
    for character in text.chars() {
        match (in_escape, character) {
            (false, '\u{1b}') => in_escape = true,
            (false, _) => plain.push(character),
            (true, 'm') => in_escape = false,
            (true, _) => {}
        }
    }
    plain
}

/// Whether some line of `text` carries every one of `parts`.
fn some_line_has(text: &str, parts: &[&str]) -> bool {
    text.lines()
        .any(|line| parts.iter().all(|part| line.contains(part)))
}

/// The keys put for [`KEYS_DID`]: 96 bytes no two neighbours alike, cut into three keys.
fn planted_bundle() -> Vec<u8> {
    (0u8..96)
        .map(|index| index.wrapping_mul(37).wrapping_add(11))
        .collect()
}

/// [`planted_bundle`] as an Account's keys, in role order.
fn planted_keys() -> AccountKeys {
    let bundle = planted_bundle();
    let (cold_recovery, rest) = bundle.split_at(32);
    let (operational, signing) = rest.split_at(32);
    AccountKeys {
        cold_recovery: SecretKey::new(cold_recovery.to_vec()),
        operational: SecretKey::new(operational.to_vec()),
        signing: SecretKey::new(signing.to_vec()),
    }
}

/// What `did`'s keys are sealed to: the custody table, then the DID.
fn keys_subject(did: &str) -> CustodySubject {
    let parts: [&[u8]; 2] = [KEYS_TABLE, did.as_bytes()];
    CustodySubject::try_from(parts.as_slice()).expect("a custody subject")
}

/// A version-1 envelope whose header names [`FORGED_KEY_ID`], over a zero nonce and tag.
fn forged_envelope() -> Vec<u8> {
    let key_id_length = u8::try_from(FORGED_KEY_ID.len()).expect("a short key id");
    let mut bytes = vec![ENVELOPE_VERSION, key_id_length];
    bytes.extend_from_slice(FORGED_KEY_ID);
    bytes.extend_from_slice(&[0u8; NONCE_BYTES + AEAD_TAG_BYTES]);
    bytes
}

/// The sealed envelope stored for `did`.
async fn stored_envelope(pool: &PgPool, did: &str) -> Vec<u8> {
    sqlx::query_scalar("SELECT sealed_keys FROM account_keys WHERE did = $1")
        .bind(did)
        .fetch_one(pool)
        .await
        .expect("the custody row is stored")
}

/// A database on the schema before the envelope, holding Alice's and Bob's 0.1 rows.
async fn before_the_envelope() -> TestDb {
    let (pool, db) = custody::before_the_envelope().await;
    custody::seed_alice_and_bob(&pool).await;
    pool.close().await;
    db
}

/// A dev [`Config`] on `database_url`, with PLC submission off.
fn config(database_url: &str) -> Config {
    Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().expect("loopback socket address"),
        public_url: "http://localhost".parse().expect("a bare origin"),
        database_url: database_url.to_owned(),
        log_level: "info".to_owned(),
        handle_domain: "zurfur.app".parse().expect("a valid handle domain"),
        plc_directory_endpoint: "https://plc.directory".to_owned(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: 60,
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        build_id: "unknown".try_into().expect("a build id"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_secret_reaches_a_trace_capture() {
    let log = CapturedLog::default();
    api::logging::subscriber(&Environment::DEV, EnvFilter::new("trace"), log.clone())
        .try_init()
        .expect("the custody canary's subscriber is this binary's only one");

    let custody = Custody::derive(
        root_of(PLANTED_ROOT),
        &Environment::DEV,
        false,
        Purpose::VARIANTS,
    )
    .expect("the guard accepts a planted dev root");
    let den_mac = custody.den_mac();
    let record_parts: [&[u8]; 1] = [DEN_RECORD];
    let record = MacMessage::try_from(record_parts.as_slice()).expect("a MAC message");
    let other_parts: [&[u8]; 1] = [b"another record"];
    let other_record = MacMessage::try_from(other_parts.as_slice()).expect("a MAC message");
    let tag = den_mac
        .mac(MacUse::DenNode, &record)
        .await
        .expect("the record is tagged");
    let verified = den_mac.verify(MacUse::DenNode, &record, &tag).await;
    let refused_tag = den_mac.verify(MacUse::DenNode, &other_record, &tag).await;

    let handles = custody::custody_of(PLANTED_ROOT);
    let (pool, _db) = test_support::pg::fresh_pool().await;
    let key_store = PgKeyStore::new(pool.clone(), handles.did_custody.clone());
    let keys_did = Did::from(KEYS_DID.to_owned());
    key_store
        .put(&keys_did, &planted_keys())
        .await
        .expect("the keys are sealed and stored");
    let reopened = key_store
        .get(&keys_did)
        .await
        .expect("the keys open")
        .expect("the keys are stored");
    let envelope = stored_envelope(&pool, KEYS_DID).await;

    let forged = SealedEnvelope::from(forged_envelope());
    let forged_open = handles
        .did_custody
        .open(&keys_subject(FORGED_DID), &forged)
        .await;
    sqlx::query("INSERT INTO account_keys (did, sealed_keys, created_at) VALUES ($1, $2, now())")
        .bind(FORGED_DID)
        .bind(forged_envelope())
        .execute(&pool)
        .await
        .expect("the forged row is planted");
    let forged_get = key_store.get(&Did::from(FORGED_DID.to_owned())).await;

    let legacy_db = before_the_envelope().await;
    let migrating = Runtime::connect_migrating(
        config(legacy_db.url()),
        root_of(LEGACY_ROOT),
        &[Purpose::DidCustody],
    );
    let (_runtime, custody_migrator) = migrating.await.expect("the migrating runtime connects");
    let checked = custody_migrator.check().await.expect("the check counts");
    let migrated = custody_migrator.run().await.expect("the migration runs");
    let wrong_db = before_the_envelope().await;
    let wrongly_migrating = Runtime::connect_migrating(
        config(wrong_db.url()),
        root_of(PLANTED_ROOT),
        &[Purpose::DidCustody],
    );
    let (_wrong_runtime, wrong_migrator) = wrongly_migrating
        .await
        .expect("the migrating runtime connects");
    let refused_run = wrong_migrator.run().await;

    custody::signin_secrets_round_trip(pool.clone(), handles.signin_secrets.clone()).await;

    assert_eq!(verified, Ok(()));
    assert_eq!(refused_tag, Err(CustodyError::Unauthentic));
    assert_eq!(reopened, planted_keys(), "the keys round-trip");
    assert_eq!(
        custody::envelope_key_id(&envelope),
        custody::key_id_of(&PLANTED_ROOT, Purpose::DidCustody),
        "the canary names the key id the custodian writes"
    );
    assert_eq!(forged_open.err(), Some(CustodyError::Malformed));
    assert!(forged_get.is_err(), "the forged envelope never opens");
    let pending_both = CustodyCheck {
        unopenable: 0,
        pending: 2,
    };
    assert_eq!(checked, pending_both);
    assert_eq!(migrated.rows_rewrapped, 2);
    assert!(
        matches!(
            refused_run,
            Err(CustodyMigrationError::Unopenable { count: 2 })
        ),
        "a wrong root refuses the run: {refused_run:?}"
    );

    let text = without_colors(&log.text());
    // A reviewer may read the capture itself: CUSTODY_TRACE_DUMP=<file> writes it there.
    if let Some(dump) = std::env::var_os("CUSTODY_TRACE_DUMP") {
        std::fs::write(dump, &text).expect("the dump file is writable");
    }
    let controls: [(&str, &[&str]); 2] = [
        (
            "the key store's span close",
            &["adapter_pg::key_store", "close"],
        ),
        (
            "the refused run's dry run",
            &["DEBUG", "a custody row fails the dry run"],
        ),
    ];
    for (control, parts) in controls {
        assert!(some_line_has(&text, parts), "no line for {control}");
    }

    let mut secrets: Vec<(String, String)> = Vec::new();
    secrets.push((
        "the root as text".to_owned(),
        String::from_utf8_lossy(&PLANTED_ROOT).into_owned(),
    ));
    secrets.extend(custody::named_printings("the root", &PLANTED_ROOT));
    secrets.extend(custody::derived_printings(&PLANTED_ROOT));
    secrets.extend(custody::named_printings("the legacy root", &LEGACY_ROOT));
    secrets.extend(
        custody::derived_printings(&LEGACY_ROOT)
            .into_iter()
            .map(|(what, printed)| (format!("{what} of the legacy root"), printed)),
    );
    secrets.extend(custody::named_printings(
        "the planted keys",
        &planted_bundle(),
    ));
    for (what, key) in [
        ("the planted cold recovery key", &reopened.cold_recovery),
        ("the planted operational key", &reopened.operational),
        ("the planted signing key", &reopened.signing),
    ] {
        secrets.extend(custody::named_printings(what, key.expose()));
    }
    let legacy = legacy_keys();
    for (what, key) in [
        ("the 0.1 cold recovery key", &legacy.cold_recovery),
        ("the 0.1 operational key", &legacy.operational),
        ("the 0.1 signing key", &legacy.signing),
    ] {
        secrets.extend(custody::named_printings(what, key.expose()));
    }
    secrets.extend(custody::named_printings("the stored envelope", &envelope));
    secrets.extend(custody::named_printings("the MAC tag", tag.stored_bytes()));
    for piece in FORGED_PIECES {
        secrets.push((format!("the forged key id's {piece:?}"), piece.to_owned()));
    }
    for (what, value) in SIGNIN_CANARY_SECRETS {
        secrets.push((format!("the sign-in's {what}"), value.to_owned()));
    }
    for (secret, value) in &secrets {
        let leaks: Vec<&str> = text
            .lines()
            .filter(|line| line.contains(value.as_str()))
            .collect();
        assert!(leaks.is_empty(), "the capture carries {secret}: {leaks:#?}");
    }

    let mut public_at_debug: Vec<String> = Purpose::VARIANTS
        .iter()
        .flat_map(|purpose| {
            [
                custody::key_id_of(&PLANTED_ROOT, *purpose),
                custody::key_id_of(&LEGACY_ROOT, *purpose),
            ]
        })
        .collect();
    let fingerprints: Vec<String> = public_at_debug
        .iter()
        .filter_map(|key_id| key_id.rsplit_once(':'))
        .map(|(_, fingerprint)| fingerprint.to_owned())
        .collect();
    public_at_debug.extend(fingerprints);
    public_at_debug.extend(
        [
            KEYS_DID,
            FORGED_DID,
            SIGNIN_CANARY_DID,
            LEGACY_ALICE_DID,
            LEGACY_BOB_DID,
            "did:plc:",
        ]
        .map(str::to_owned),
    );
    let at_info: Vec<&str> = text
        .lines()
        .filter(|line| matches!(level_of(line), Some("INFO" | "WARN" | "ERROR")))
        .filter(|line| {
            public_at_debug
                .iter()
                .any(|public| line.contains(public.as_str()))
        })
        .collect();
    assert!(
        at_info.is_empty(),
        "an info-or-above line names a key id or a DID: {at_info:#?}"
    );
}
