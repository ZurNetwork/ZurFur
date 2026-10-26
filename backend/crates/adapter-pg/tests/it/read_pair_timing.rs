//! The clock time of the commission read at the store, with no HTTP in front of it: the
//! summary as the viewer reads it, one statement on the pool store. A stranger
//! asks about a Private commission with an ordinary name, one whose 2048-byte name is stored
//! out of line, and a made-up id; the paired differences are printed with their bootstrap
//! 95% intervals. Not a gate:
//! `cargo test -p adapter-pg --test it --release read_pair_timing -- --ignored --nocapture`,
//! with `READ_PAIR_TIMING_RUNS` to change the run count. Requires a container runtime socket.

use std::time::{Duration, Instant};

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use domain::{
    elements::{
        commission::CommissionId,
        did::Did,
        user::{User, UserId},
    },
    ports::{CommissionStore, Database},
};
use test_support::timing::{BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, Iteration, PairedSummary};

use crate::seed;

/// Timed runs of each cause, unless `READ_PAIR_TIMING_RUNS` says otherwise.
const DEFAULT_RUNS: usize = 5_000;

/// Untimed runs of each cause before the timed ones.
const WARM_UP_RUNS: usize = 50;

/// How long the large name is, in bytes.
const LARGE_NAME_BYTES: usize = 2048;

/// The characters the large name is drawn from.
const NAME_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// A made-up id: a UUIDv7 from 2025, before any row's.
const MADE_UP_ID: uuid::Uuid = uuid::Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0b01);

/// The stranger's DID: a user who takes part in nothing.
const STRANGER_DID: &str = "did:plc:readpairstrangeraaaaaaaa";

/// The owner's DID.
const OWNER_DID: &str = "did:plc:readpairowneraaaaaaaaaaa";

/// A name of [`LARGE_NAME_BYTES`] bytes that does not compress, so the store keeps it out of
/// line: splitmix64 output over [`NAME_ALPHABET`].
fn large_name() -> String {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut name = String::with_capacity(LARGE_NAME_BYTES);
    while name.len() < LARGE_NAME_BYTES {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^= mixed >> 31;
        let index = usize::try_from(mixed % NAME_ALPHABET.len() as u64).expect("an index");
        name.push(char::from(NAME_ALPHABET[index]));
    }
    name
}

/// Recognizes `did` as a user, committed.
async fn provision(pool: &PgPool, did: &str) -> User {
    let database = PgDatabase::new(pool.clone());
    let mut unit = database.begin().await.expect("begin");
    let user = unit
        .users()
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision");
    unit.commit().await.expect("commit");
    user
}

/// Stores `name` as the name of the node `commission` points at, a test-only update.
async fn store_node_name(pool: &PgPool, commission: CommissionId, name: &str) {
    let updated = sqlx::query(
        "UPDATE node SET name = $1 FROM commission c WHERE c.id = $2 AND node.id = c.node_id",
    )
    .bind(name)
    .bind(uuid::Uuid::from(commission))
    .execute(pool)
    .await
    .expect("update the commission node's name");
    assert_eq!(updated.rows_affected(), 1, "one node is updated");
}

/// The timed unit: what one read of a commission asks the store, its summary as `viewer`
/// reads it. Before the one-statement read it was the summary and then whether `viewer`
/// takes part, two statements.
async fn read_pair(store: &PgCommissionStore, commission: &CommissionId, viewer: &UserId) {
    let viewed = store
        .find_summary(commission, viewer)
        .await
        .expect("find_summary");
    std::hint::black_box(viewed);
}

/// One timed `read_pair`.
async fn timed(store: &PgCommissionStore, commission: &CommissionId, viewer: &UserId) -> Duration {
    let started = Instant::now();
    read_pair(store, commission, viewer).await;
    started.elapsed()
}

/// The three causes, in the order an iteration times them: rotated each run so none always
/// follows the same other.
fn rotated<T: Copy>(causes: [T; 3], run: usize) -> [T; 3] {
    let start = run % 3;
    [
        causes[start],
        causes[(start + 1) % 3],
        causes[(start + 2) % 3],
    ]
}

/// Which cause a timing belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    MadeUp,
    Ordinary,
    Large,
}

/// Prints `label`'s paired differences against the made-up id: median and bootstrap 95% CI.
fn report(label: &str, iterations: &[Iteration]) {
    let summary = PairedSummary::of(iterations);
    println!(
        "{label}: median {:+.1} µs, 95% CI [{:+.1}, {:+.1}] µs",
        summary.median.as_f64(),
        summary.interval.low.as_f64(),
        summary.interval.high.as_f64(),
    );
}

#[tokio::test]
#[ignore = "clock times, not a gate: run with --release -- --ignored --nocapture"]
async fn the_read_pair_of_a_stranger_s_private_commission_against_a_made_up_id() {
    let runs = std::env::var("READ_PAIR_TIMING_RUNS")
        .ok()
        .and_then(|runs| runs.parse().ok())
        .unwrap_or(DEFAULT_RUNS);
    let (pool, _container) = test_support::pg::fresh_pool().await;
    let owner = provision(&pool, OWNER_DID).await;
    let ordinary = seed::seed_titled(&pool, &owner.id, "An ordinary name").await;
    let large = seed::seed_titled(&pool, &owner.id, "Soon a large name").await;
    store_node_name(&pool, large, &large_name()).await;
    let made_up = CommissionId::from(MADE_UP_ID);
    let stranger = UserId::from(Did::from(STRANGER_DID.to_owned()));
    let store = PgCommissionStore::new(pool.clone());

    for _ in 0..WARM_UP_RUNS {
        for commission in [&made_up, &ordinary, &large] {
            timed(&store, commission, &stranger).await;
        }
    }
    let mut ordinary_runs = Vec::with_capacity(runs);
    let mut large_runs = Vec::with_capacity(runs);
    for run in 0..runs {
        let mut made_up_time = Duration::ZERO;
        let mut ordinary_time = Duration::ZERO;
        let mut large_time = Duration::ZERO;
        for cause in rotated([Cause::MadeUp, Cause::Ordinary, Cause::Large], run) {
            match cause {
                Cause::MadeUp => made_up_time = timed(&store, &made_up, &stranger).await,
                Cause::Ordinary => ordinary_time = timed(&store, &ordinary, &stranger).await,
                Cause::Large => large_time = timed(&store, &large, &stranger).await,
            }
        }
        ordinary_runs.push(Iteration {
            absent: made_up_time,
            not_yours: ordinary_time,
        });
        large_runs.push(Iteration {
            absent: made_up_time,
            not_yours: large_time,
        });
    }

    println!(
        "{runs} interleaved iterations, after {WARM_UP_RUNS} untimed; bootstrap: \
         {BOOTSTRAP_RESAMPLES} resamples, seed {BOOTSTRAP_SEED:#x}"
    );
    report(
        "a stranger's Private commission, ordinary name",
        &ordinary_runs,
    );
    report(
        "a stranger's Private commission, 2048-byte name",
        &large_runs,
    );
}
