//! Clock times of each pair of not-found causes on PostgreSQL, for the security review. Each
//! pair is timed over many runs that put its two causes together in every iteration, in
//! alternating order; the signed difference of the two (the not-yours cause's time minus the
//! made-up id's) is the observation, and its median and bootstrap 95% interval are what the
//! table reports. The pairs: every read pair the statement suite runs, from a warm and a cold
//! profile cache; the writes (the rename and the new field writes), against which the rest
//! are read; every read, call and board card of the router's table over the stranger's
//! targets; and two made-up ids against each other (A/A), which say how noisy the run was.
//! Not gated, as clock times are too noisy for CI: run it with `just den-timing`. Requires a
//! container runtime socket.

use std::time::{Duration, Instant};

use strum::VariantArray as _;
use test_support::{
    den::get,
    pg_app::{
        self, CommissionCall, DenWorld, MissPair, ProfileCacheState, SeededCommission,
        StrangerBoard, archived_participant_pairs, commission_read_live_pairs,
        commission_read_stranger_pairs, den_account_pair, den_character_pair,
        den_participant_pairs, den_stranger_pairs, den_stranger_targets, download_participant_pair,
        every_commission_call, new_write_calls, placement_call, read_call, rename_call,
        rename_stranger_pair, stranger_targets,
    },
    timing::{BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, Interval, Iteration, Micros, PairedSummary},
};

use domain::elements::{commission::CommissionId, level::Level};

/// Timed runs of each pair, unless `DEN_TIMING_RUNS` says otherwise.
const DEFAULT_RUNS: usize = 200;

/// Untimed runs of each cause before the timed ones, so the pool's connections have every
/// statement prepared.
const WARM_UP_RUNS: usize = 5;

/// What a pair is read against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
enum Class {
    /// The gate's own writes: the band every gated pair is read against.
    #[strum(serialize = "write")]
    Write,
    /// A pair over a Private live target or any archived one: its causes must not differ.
    #[strum(serialize = "gated")]
    Gated,
    /// A pair over a live Listed or Public target, where existence is public: reported only.
    #[strum(serialize = "exempt")]
    Exempt,
    /// Two made-up ids: the run's own noise.
    #[strum(serialize = "A/A")]
    AA,
}

/// How a pair's causes are sent.
#[derive(Clone, Copy)]
enum Request<'a> {
    /// A plain `GET`.
    Get,
    /// A commission call.
    Call(&'a CommissionCall),
}

/// The status each cause of a pair must answer.
#[derive(Debug, Clone, Copy)]
struct Statuses {
    absent: u16,
    not_yours: u16,
}

impl Statuses {
    /// Both causes answer the one not-found.
    const NOT_FOUND: Statuses = Statuses {
        absent: 404,
        not_yours: 404,
    };
}

/// Which cause of a pair a request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    /// The made-up id.
    Absent,
    /// The real thing the viewer may not open.
    NotYours,
}

/// A card the not-yours cause puts on a board, to be taken off after each request outside the
/// timing: a column takes a commission once.
#[derive(Clone, Copy)]
struct PlacedCard<'a> {
    board: &'a StrangerBoard,
    commission: CommissionId,
}

/// One pair to time.
struct Spec<'a> {
    label: String,
    class: Class,
    viewer: &'a reqwest::Client,
    request: Request<'a>,
    pair: &'a MissPair,
    /// The profile cache state each run starts from, or `None` to leave it as it is.
    cache: Option<ProfileCacheState>,
    statuses: Statuses,
    /// The card the not-yours cause leaves on a board, if it places one.
    placed: Option<PlacedCard<'a>>,
}

/// One table row: a pair's two causes' medians and its paired difference.
struct Measured {
    label: String,
    class: Class,
    absent_median: Micros,
    not_yours_median: Micros,
    summary: PairedSummary,
    /// The first answer that was not the status the pair expects; empty when all were.
    note: String,
}

/// One timed request: how long it took and what status it was answered.
struct Timed {
    elapsed: Duration,
    status: u16,
}

/// The median of `times`, which must not be empty.
fn median_of(times: impl Iterator<Item = Duration>) -> Micros {
    let micros: Vec<Micros> = times.map(Micros::from).collect();
    test_support::timing::median(&micros)
}

/// Times pairs against one served world.
struct Runner<'a> {
    world: &'a DenWorld,
    runs: usize,
    rows: Vec<Measured>,
}

impl Runner<'_> {
    /// Times `spec`'s pair: warm-up runs, then `runs` iterations that each time both causes,
    /// in alternating order so neither always follows the other. A status other than the one
    /// the pair expects does not stop the run: the first is noted in the pair's row, since
    /// causes that answer differently are not a pair to read the clock of.
    async fn time(&mut self, spec: Spec<'_>) {
        for _ in 0..WARM_UP_RUNS {
            self.run(&spec, Cause::Absent).await;
            self.run(&spec, Cause::NotYours).await;
        }
        let mut iterations = Vec::with_capacity(self.runs);
        let mut note = String::new();
        for run in 0..self.runs {
            let (absent, not_yours) = if run % 2 == 0 {
                let absent = self.run(&spec, Cause::Absent).await;
                let not_yours = self.run(&spec, Cause::NotYours).await;
                (absent, not_yours)
            } else {
                let not_yours = self.run(&spec, Cause::NotYours).await;
                let absent = self.run(&spec, Cause::Absent).await;
                (absent, not_yours)
            };
            let answers = [
                ("absent", absent.status, spec.statuses.absent),
                ("not yours", not_yours.status, spec.statuses.not_yours),
            ];
            for (cause, answered, expected) in answers {
                if answered != expected && note.is_empty() {
                    note = format!("{cause} answered {answered}, expected {expected}");
                }
            }
            let iteration = Iteration {
                absent: absent.elapsed,
                not_yours: not_yours.elapsed,
            };
            iterations.push(iteration);
        }
        let measured = Measured {
            label: spec.label,
            class: spec.class,
            absent_median: median_of(iterations.iter().map(|run| run.absent)),
            not_yours_median: median_of(iterations.iter().map(|run| run.not_yours)),
            summary: PairedSummary::of(&iterations),
            note,
        };
        self.rows.push(measured);
    }

    /// One request of `spec`'s `cause`, timed from the request to the body's last byte; the
    /// cache is set outside the timing, and so is taking a placed card off again.
    async fn run(&self, spec: &Spec<'_>, cause: Cause) -> Timed {
        let path = match cause {
            Cause::Absent => &spec.pair.absent,
            Cause::NotYours => &spec.pair.not_yours,
        };
        if let Some(cache) = spec.cache {
            self.world.set_profile_cache(cache).await;
        }
        let base = self.world.base_url();
        let started = Instant::now();
        let response = match spec.request {
            Request::Get => get(spec.viewer, base, path).await,
            Request::Call(call) => call.send(spec.viewer, base, path).await,
        };
        let status = response.status().as_u16();
        let _body = response.bytes().await.expect("the body");
        let elapsed = started.elapsed();
        if let (Cause::NotYours, Some(card)) = (cause, spec.placed) {
            self.world
                .take_stranger_card_off(card.board, card.commission)
                .await;
        }
        Timed { elapsed, status }
    }

    /// Times `pair` by `GET` as `viewer` from each cache state.
    async fn time_reads(
        &mut self,
        viewer: &reqwest::Client,
        label: &str,
        class: Class,
        pair: &MissPair,
    ) {
        for &cache in ProfileCacheState::VARIANTS {
            let spec = Spec {
                label: format!("{label} [{cache}]"),
                class,
                viewer,
                request: Request::Get,
                pair,
                cache: Some(cache),
                statuses: Statuses::NOT_FOUND,
                placed: None,
            };
            self.time(spec).await;
        }
    }

    /// Times `call` on `pair` as `viewer`, leaving the profile cache as it is.
    async fn time_call(
        &mut self,
        viewer: &reqwest::Client,
        label: String,
        class: Class,
        call: &CommissionCall,
        pair: &MissPair,
    ) {
        let spec = Spec {
            label,
            class,
            viewer,
            request: Request::Call(call),
            pair,
            cache: None,
            statuses: Statuses::NOT_FOUND,
            placed: None,
        };
        self.time(spec).await;
    }
}

/// How a read pair over `target` at `visibility` is read: where the node is live and
/// Listed or Public, existence is public and the pair is reported only.
fn read_class(world: &DenWorld, target: &SeededCommission, visibility: Level) -> Class {
    let live = world.archived_or_live(target) == "live";
    let existence_is_public = matches!(visibility, Level::Listed | Level::Public);
    if live && existence_is_public {
        Class::Exempt
    } else {
        Class::Gated
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

/// The lowest CI low and the highest CI high among the write rows.
fn write_band(rows: &[Measured]) -> Interval {
    let writes = rows.iter().filter(|row| row.class == Class::Write);
    let mut lows = Vec::new();
    let mut highs = Vec::new();
    for row in writes {
        lows.push(row.summary.interval.low.as_f64());
        highs.push(row.summary.interval.high.as_f64());
    }
    let low = lows.into_iter().fold(f64::INFINITY, f64::min);
    let high = highs.into_iter().fold(f64::NEG_INFINITY, f64::max);
    Interval {
        low: Micros::from(low),
        high: Micros::from(high),
    }
}

/// The row as a Markdown table line, in microseconds; a gated row says whether its CI high is
/// at or below the write band's.
fn line(row: &Measured, band: &Interval) -> String {
    let within = match row.class {
        Class::Gated if row.summary.interval.high <= band.high => "yes",
        Class::Gated => "NO",
        Class::Write | Class::Exempt | Class::AA => "-",
    };
    format!(
        "| {} | {} | {:.0} | {:.0} | {:+.0} | {:+.0} | {:+.0} | {} | {} |",
        row.label,
        row.class,
        row.absent_median.as_f64(),
        row.not_yours_median.as_f64(),
        row.summary.median.as_f64(),
        row.summary.interval.low.as_f64(),
        row.summary.interval.high.as_f64(),
        within,
        row.note,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "clock times for the security review, not a gate: run with `just den-timing`"]
async fn clock_times_of_each_pair_s_causes() {
    let runs = std::env::var("DEN_TIMING_RUNS")
        .ok()
        .and_then(|runs| runs.parse().ok())
        .unwrap_or(DEFAULT_RUNS);
    let world = pg_app::den_world(api::app).await;
    let board = world.seed_stranger_board().await;
    let column = board.column;
    let load_before = load_average();
    world.set_profile_cache(ProfileCacheState::Warm).await;
    let mut runner = Runner {
        world: &world,
        runs,
        rows: Vec::new(),
    };

    // bob takes no part in the Private "Old sketch" yet, so each call there is his 404.
    let rename_pair = rename_stranger_pair(&world);
    let rename = rename_call();
    let label = format!("{}, bob", rename.label);
    runner
        .time_call(&world.bob, label, Class::Write, &rename, &rename_pair)
        .await;
    for call in new_write_calls() {
        let pair = call.pair(&world.old_sketch);
        let label = format!("{}, bob", call.label);
        runner
            .time_call(&world.bob, label, Class::Write, &call, &pair)
            .await;
    }
    let read = read_call();
    let read_pair = read.pair(&world.old_sketch);
    let read_label = format!("{}, bob", read.label);
    runner
        .time_call(&world.bob, read_label, Class::Gated, &read, &read_pair)
        .await;

    // Two made-up ids against each other: nothing differs, so the interval must hold zero.
    let archive = every_commission_call(column)
        .into_iter()
        .find(|call| call.label == "archive")
        .expect("the table holds the archive");
    for call in [read_call(), archive] {
        let pair = call.absent_pair();
        let label = format!("A/A: {}, stranger", call.label);
        runner
            .time_call(&world.stranger, label, Class::AA, &call, &pair)
            .await;
    }

    for (target, visibility) in den_stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        let class = read_class(&world, &target, visibility);
        for pair in den_stranger_pairs(&target) {
            let label = format!("stranger, den, {visibility} {lifecycle}, {}", pair.label);
            runner
                .time_reads(&world.stranger, &label, class, &pair)
                .await;
        }
    }
    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        let class = read_class(&world, &target, visibility);
        for pair in commission_read_stranger_pairs(&target) {
            let label = format!("stranger, {visibility} {lifecycle}, {}", pair.label);
            runner
                .time_reads(&world.stranger, &label, class, &pair)
                .await;
        }
    }
    let account = den_account_pair(&world);
    let account_label = format!("stranger, den, {}", account.label);
    runner
        .time_reads(&world.stranger, &account_label, Class::Gated, &account)
        .await;
    let characters = den_character_pair();
    let characters_label = format!("stranger, den, {}", characters.label);
    runner
        .time_reads(
            &world.stranger,
            &characters_label,
            Class::Gated,
            &characters,
        )
        .await;
    for (visibility, pair, _) in commission_read_live_pairs(&world) {
        world.set_visibility(world.untitled.id, visibility).await;
        let label = format!("stranger, {visibility} live, {}", pair.label);
        runner
            .time_reads(&world.stranger, &label, Class::Exempt, &pair)
            .await;
    }
    for pair in den_participant_pairs(&world) {
        let label = format!("bob, den, {}", pair.label);
        runner
            .time_reads(&world.bob, &label, Class::Gated, &pair)
            .await;
    }
    let download = download_participant_pair(&world);
    let download_label = format!("bob, {}", download.label);
    runner
        .time_reads(&world.bob, &download_label, Class::Gated, &download)
        .await;

    // Every read, call and board card of the router's table, as the stranger with a board.
    let table = every_commission_call(column);
    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        for call in &table {
            let pair = call.pair(&target);
            let label = format!(
                "stranger, {visibility} {lifecycle}, {} ({})",
                call.label, call.kind
            );
            runner
                .time_call(&world.stranger, label, Class::Gated, call, &pair)
                .await;
        }
    }
    // The card of a live Listed or Public commission: placed (204) against a made-up id's 404.
    let placement = placement_call(column);
    for visibility in [Level::Listed, Level::Public] {
        world.set_visibility(world.untitled.id, visibility).await;
        let pair = placement.pair(&world.untitled);
        let spec = Spec {
            label: format!(
                "stranger, {visibility} live, {} (placement)",
                placement.label
            ),
            class: Class::Exempt,
            viewer: &world.stranger,
            request: Request::Call(&placement),
            pair: &pair,
            cache: None,
            statuses: Statuses {
                absent: 404,
                not_yours: 204,
            },
            placed: Some(PlacedCard {
                board: &board,
                commission: world.untitled.id,
            }),
        };
        runner.time(spec).await;
    }

    world.add_bob_to_old_sketch().await;
    for pair in archived_participant_pairs(&world) {
        let label = format!("bob, {}", pair.label);
        runner
            .time_reads(&world.bob, &label, Class::Gated, &pair)
            .await;
    }

    let load_after = load_average();
    let cpus = std::thread::available_parallelism().map_or(0, usize::from);
    let rows = runner.rows;
    let band = write_band(&rows);
    println!(
        "{runs} timed iterations per pair, each timing both causes, after {WARM_UP_RUNS} untimed; \
         bootstrap: {BOOTSTRAP_RESAMPLES} resamples, seed {BOOTSTRAP_SEED:#x}"
    );
    println!("CPUs: {cpus}; load average before: {load_before}; after: {load_after}");
    println!();
    println!(
        "| Pair | Class | Absent median µs | Not yours median µs | Δ median µs (not yours − absent) | CI low µs | CI high µs | Within the write band | Note |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for row in &rows {
        println!("{}", line(row, &band));
    }
    println!();
    println!(
        "Write band: CI low {:+.0} µs, CI high {:+.0} µs",
        band.low.as_f64(),
        band.high.as_f64(),
    );
    println!();
    let zero = Micros::from(0.0);
    for row in rows.iter().filter(|row| row.class == Class::AA) {
        let verdict = if row.summary.interval.contains(zero) {
            "its CI includes 0"
        } else {
            "its CI excludes 0: too noisy: repeat"
        };
        println!("{}: {verdict}", row.label);
    }
}
