//! Clock times of each pair of not-found causes on PostgreSQL, for the security review: every
//! read pair the statement suite runs, each cause timed over many runs from a warm and a cold
//! profile cache, printed as a Markdown table; then the rename's pair, interleaved and warm,
//! with each cause's median, p10 and p90 and the pass rule's verdict. Not gated, as clock times
//! are too noisy for CI: run it with `just den-timing`. Requires a container runtime socket.

use std::time::{Duration, Instant};

use strum::VariantArray as _;
use test_support::{
    den::{get, patch_title},
    pg_app::{
        self, DenWorld, MissPair, ProfileCacheState, RENAME_TITLE, archived_participant_pairs,
        commission_read_live_pairs, commission_read_stranger_pairs, den_account_pair,
        den_character_pair, den_participant_pairs, den_stranger_pairs, den_stranger_targets,
        download_participant_pair, rename_stranger_pair, stranger_targets,
    },
};

/// Timed runs of each read cause per cache state, and timed pairs of the write's causes,
/// unless `DEN_TIMING_RUNS` says otherwise.
const DEFAULT_RUNS: usize = 200;

/// Untimed runs of each cause before the timed ones, so the pool's connections have every
/// statement prepared.
const WARM_UP_RUNS: usize = 5;

/// The most the two causes' medians may differ by, as a share of the smaller, for the write
/// pair to pass.
const MEDIAN_TOLERANCE: f64 = 0.05;

/// The median, the 10th, 90th and 95th percentiles of one cause's runs.
#[derive(Debug, Clone, Copy)]
struct Spread {
    median: Duration,
    p10: Duration,
    p90: Duration,
    p95: Duration,
}

/// The nearest-rank `percent`th percentile of `sorted`, which must not be empty.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    sorted[rank - 1]
}

impl Spread {
    /// The spread of `runs`, which must not be empty.
    fn of(mut runs: Vec<Duration>) -> Self {
        runs.sort();
        Self {
            median: runs[runs.len() / 2],
            p10: percentile(&runs, 10),
            p90: percentile(&runs, 90),
            p95: percentile(&runs, 95),
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

/// The write pair's two causes, each over the same interleaved runs, and the pass rule.
struct WritePair {
    absent: Spread,
    not_yours: Spread,
}

impl WritePair {
    /// Times `pair` as `viewer` through the rename: in each iteration both causes run, in
    /// alternating order, so drift lands on both alike.
    async fn time(
        world: &DenWorld,
        viewer: &reqwest::Client,
        pair: &MissPair,
        runs: usize,
    ) -> Self {
        for _ in 0..WARM_UP_RUNS {
            rename(world, viewer, &pair.absent).await;
            rename(world, viewer, &pair.not_yours).await;
        }
        let mut absent = Vec::with_capacity(runs);
        let mut not_yours = Vec::with_capacity(runs);
        for run in 0..runs {
            if run % 2 == 0 {
                absent.push(rename(world, viewer, &pair.absent).await);
                not_yours.push(rename(world, viewer, &pair.not_yours).await);
            } else {
                not_yours.push(rename(world, viewer, &pair.not_yours).await);
                absent.push(rename(world, viewer, &pair.absent).await);
            }
        }
        Self {
            absent: Spread::of(absent),
            not_yours: Spread::of(not_yours),
        }
    }

    /// Whether each cause's p10 is at or below the other's p90.
    fn ranges_overlap(&self) -> bool {
        self.absent.p10 <= self.not_yours.p90 && self.not_yours.p10 <= self.absent.p90
    }

    /// How far apart the medians are, as a share of the smaller.
    fn median_gap(&self) -> f64 {
        let absent = micros(self.absent.median);
        let not_yours = micros(self.not_yours.median);
        (absent - not_yours).abs() / absent.min(not_yours)
    }

    /// Whether the pair passes: the ranges overlap and the medians are within tolerance.
    fn passes(&self) -> bool {
        self.ranges_overlap() && self.median_gap() < MEDIAN_TOLERANCE
    }

    /// The pair's table and its verdict, in microseconds.
    fn report(&self, runs: usize) -> Vec<String> {
        let line = |cause: &str, spread: &Spread| {
            format!(
                "| rename, {cause} | {:.0} | {:.0} | {:.0} |",
                micros(spread.median),
                micros(spread.p10),
                micros(spread.p90),
            )
        };
        let verdict = if self.passes() {
            "pass"
        } else {
            "FINDING: report the numbers to the security reviewer"
        };
        vec![
            format!(
                "Write pair: bob's rename, absent against not yours, {runs} interleaved pairs, warm"
            ),
            String::new(),
            "| Cause | Median µs | p10 µs | p90 µs |".to_owned(),
            "|---|---|---|---|".to_owned(),
            line("absent", &self.absent),
            line("not yours", &self.not_yours),
            String::new(),
            format!(
                "Each p10 at or below the other's p90: {}",
                self.ranges_overlap()
            ),
            format!(
                "Medians apart: {:.1}% of the smaller (under {:.0}%: {})",
                self.median_gap() * 100.0,
                MEDIAN_TOLERANCE * 100.0,
                self.median_gap() < MEDIAN_TOLERANCE,
            ),
            format!("Write pair verdict: {verdict}"),
        ]
    }
}

/// One `PATCH {path}` with [`RENAME_TITLE`] as `viewer`, timed from the request to the body's
/// last byte.
async fn rename(world: &DenWorld, viewer: &reqwest::Client, path: &str) -> Duration {
    let started = Instant::now();
    let response = patch_title(viewer, world.base_url(), path, RENAME_TITLE).await;
    let status = response.status();
    let _body = response.bytes().await.expect("the body");
    let elapsed = started.elapsed();
    assert_eq!(status, 404, "{path}");
    elapsed
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
    world.set_profile_cache(ProfileCacheState::Warm).await;
    let rename_pair = rename_stranger_pair(&world);
    let write_pair = WritePair::time(&world, &world.bob, &rename_pair, runs).await;
    let mut timer = Timer {
        world: &world,
        runs,
        rows: Vec::new(),
    };

    for (target, visibility) in den_stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in den_stranger_pairs(&target) {
            let label = format!("stranger, den, {visibility} {lifecycle}, {}", pair.label);
            timer.time(&world.stranger, label, &pair).await;
        }
    }
    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
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
        world.set_visibility(world.untitled.id, visibility).await;
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
    println!();
    for line in write_pair.report(runs) {
        println!("{line}");
    }
}
