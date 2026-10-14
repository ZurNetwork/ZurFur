//! Clock times of each pair of not-found causes on PostgreSQL, for the security review: every
//! pair the statement suite runs, each cause timed over many runs from a warm and a cold profile
//! cache, printed as a Markdown table. Not gated, as clock times are too noisy for CI: run it
//! with `just den-timing`. Requires a container runtime socket.

use std::time::{Duration, Instant};

use strum::VariantArray as _;
use test_support::{
    den::get,
    pg_app::{
        self, DenWorld, MissPair, ProfileCacheState, archived_participant_pairs,
        commission_read_live_pairs, commission_read_stranger_pairs, den_account_pair,
        den_character_pair, den_participant_pairs, den_stranger_pairs, den_stranger_targets,
        download_participant_pair, stranger_targets,
    },
};

/// Timed runs of each cause per cache state, unless `DEN_TIMING_RUNS` says otherwise.
const DEFAULT_RUNS: usize = 200;

/// Untimed runs of each cause before the timed ones, so the pool's connections have every
/// statement prepared.
const WARM_UP_RUNS: usize = 5;

/// The median and the 95th percentile of one cause's runs.
#[derive(Debug, Clone, Copy)]
struct Spread {
    median: Duration,
    p95: Duration,
}

impl Spread {
    /// The spread of `runs`, which must not be empty.
    fn of(mut runs: Vec<Duration>) -> Self {
        runs.sort();
        let median = runs[runs.len() / 2];
        let p95_at = (runs.len() * 95).div_ceil(100) - 1;
        Self {
            median,
            p95: runs[p95_at],
        }
    }
}

/// One table row: a pair's two causes from one cache state.
struct Row {
    label: String,
    cache: ProfileCacheState,
    absent: Spread,
    not_yours: Spread,
}

/// `duration` in microseconds.
fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

impl Row {
    /// The row as a Markdown table line, in microseconds.
    fn line(&self) -> String {
        let difference = micros(self.not_yours.median) - micros(self.absent.median);
        format!(
            "| {} | {} | {:.0} | {:.0} | {:.0} | {:.0} | {:+.0} |",
            self.label,
            self.cache,
            micros(self.absent.median),
            micros(self.absent.p95),
            micros(self.not_yours.median),
            micros(self.not_yours.p95),
            difference,
        )
    }
}

/// Times both causes of `pair` as `viewer`, from each cache state, into `rows`.
struct Timer<'a> {
    world: &'a DenWorld,
    runs: usize,
    rows: Vec<Row>,
}

impl Timer<'_> {
    async fn time(&mut self, viewer: &reqwest::Client, label: String, pair: &MissPair) {
        for &cache in ProfileCacheState::VARIANTS {
            for _ in 0..WARM_UP_RUNS {
                self.run(viewer, cache, &pair.absent).await;
                self.run(viewer, cache, &pair.not_yours).await;
            }
            let mut absent = Vec::with_capacity(self.runs);
            let mut not_yours = Vec::with_capacity(self.runs);
            for run in 0..self.runs {
                // Alternate which cause goes first, so neither always follows the other.
                if run % 2 == 0 {
                    absent.push(self.run(viewer, cache, &pair.absent).await);
                    not_yours.push(self.run(viewer, cache, &pair.not_yours).await);
                } else {
                    not_yours.push(self.run(viewer, cache, &pair.not_yours).await);
                    absent.push(self.run(viewer, cache, &pair.absent).await);
                }
            }
            let row = Row {
                label: label.clone(),
                cache,
                absent: Spread::of(absent),
                not_yours: Spread::of(not_yours),
            };
            self.rows.push(row);
        }
    }

    /// One `GET {path}` as `viewer` from `cache`, timed from the request to the body's last
    /// byte; the cache is set outside the timing.
    async fn run(
        &self,
        viewer: &reqwest::Client,
        cache: ProfileCacheState,
        path: &str,
    ) -> Duration {
        self.world.set_profile_cache(cache).await;
        let started = Instant::now();
        let response = get(viewer, self.world.base_url(), path).await;
        let status = response.status();
        let _body = response.bytes().await.expect("the body");
        let elapsed = started.elapsed();
        assert_eq!(status, 404, "{path}");
        elapsed
    }
}

/// The machine's one-, five- and fifteen-minute load averages.
fn load_average() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|load| {
            load.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|_| "unknown".to_owned())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "clock times for the security review, not a gate: run with `just den-timing`"]
async fn clock_times_of_each_pair_s_causes() {
    let runs = std::env::var("DEN_TIMING_RUNS")
        .ok()
        .and_then(|runs| runs.parse().ok())
        .unwrap_or(DEFAULT_RUNS);
    let world = pg_app::den_world(api::app).await;
    let load_before = load_average();
    let mut timer = Timer {
        world: &world,
        runs,
        rows: Vec::new(),
    };

    for (target, visibility) in den_stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in den_stranger_pairs(&target) {
            let label = format!("stranger, den, {visibility} {lifecycle}, {}", pair.label);
            timer.time(&world.stranger, label, &pair).await;
        }
    }
    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in commission_read_stranger_pairs(&target) {
            let label = format!("stranger, {visibility} {lifecycle}, {}", pair.label);
            timer.time(&world.stranger, label, &pair).await;
        }
    }
    let account = den_account_pair(&world);
    let account_label = format!("stranger, den, {}", account.label);
    timer.time(&world.stranger, account_label, &account).await;
    let characters = den_character_pair();
    let characters_label = format!("stranger, den, {}", characters.label);
    timer
        .time(&world.stranger, characters_label, &characters)
        .await;
    for (visibility, pair, _) in commission_read_live_pairs(&world) {
        world
            .set_visibility(world.untitled.id, visibility.clone())
            .await;
        let label = format!("stranger, {visibility} live, {}", pair.label);
        timer.time(&world.stranger, label, &pair).await;
    }
    for pair in den_participant_pairs(&world) {
        let label = format!("bob, den, {}", pair.label);
        timer.time(&world.bob, label, &pair).await;
    }
    let download = download_participant_pair(&world);
    let download_label = format!("bob, {}", download.label);
    timer.time(&world.bob, download_label, &download).await;
    world.add_bob_to_old_sketch().await;
    for pair in archived_participant_pairs(&world) {
        let label = format!("bob, {}", pair.label);
        timer.time(&world.bob, label, &pair).await;
    }

    let load_after = load_average();
    let cpus = std::thread::available_parallelism().map_or(0, usize::from);
    println!("{runs} timed runs per cause per cache state, after {WARM_UP_RUNS} untimed");
    println!("CPUs: {cpus}; load average before: {load_before}; after: {load_after}");
    println!();
    println!(
        "| Pair | Cache | Absent median µs | Absent p95 µs | Not yours median µs | Not yours p95 µs | Δ median µs |"
    );
    println!("|---|---|---|---|---|---|---|");
    for row in &timer.rows {
        println!("{}", row.line());
    }
}
