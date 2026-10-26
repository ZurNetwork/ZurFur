//! The statistics the not-found timing runs read their pairs by: each iteration times both
//! causes together, the signed difference of the two is the observation, and a seeded
//! bootstrap puts a 95% interval around the median of those differences.

use std::time::Duration;

use rand::{Rng as _, SeedableRng as _, rngs::StdRng};

/// The seed of every bootstrap, so one series always has one interval.
pub const BOOTSTRAP_SEED: u64 = 0x2026_1026_0F0F_5EED;

/// How many resamples, with replacement, one bootstrap draws.
pub const BOOTSTRAP_RESAMPLES: usize = 10_000;

/// The lower bound's percentile, in tenths of a percent: the 2.5th.
const LOWER_TENTHS: usize = 25;

/// The upper bound's percentile, in tenths of a percent: the 97.5th.
const UPPER_TENTHS: usize = 975;

/// A time, or a difference of times, in microseconds; signed.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Micros(f64);

impl Micros {
    /// The microseconds as a plain number, for printing.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl From<Duration> for Micros {
    fn from(duration: Duration) -> Self {
        Self(duration.as_secs_f64() * 1e6)
    }
}

impl From<f64> for Micros {
    fn from(micros: f64) -> Self {
        Self(micros)
    }
}

impl std::ops::Sub for Micros {
    type Output = Micros;

    fn sub(self, other: Micros) -> Micros {
        Micros(self.0 - other.0)
    }
}

/// One iteration of a pair: both causes timed back to back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Iteration {
    /// The made-up id's time.
    pub absent: Duration,
    /// The real thing the viewer may not open: its time.
    pub not_yours: Duration,
}

impl Iteration {
    /// The signed difference, the real thing's time minus the made-up id's.
    pub fn difference(&self) -> Micros {
        Micros::from(self.not_yours) - Micros::from(self.absent)
    }
}

/// A closed range of microseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    /// The lower bound.
    pub low: Micros,
    /// The upper bound.
    pub high: Micros,
}

impl Interval {
    /// Whether `value` lies in the range, bounds included.
    pub fn contains(&self, value: Micros) -> bool {
        self.low <= value && value <= self.high
    }
}

/// A pair's paired-difference median and its bootstrap 95% interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairedSummary {
    /// The median of the signed differences.
    pub median: Micros,
    /// The interval the median lies in, at 95%.
    pub interval: Interval,
}

impl PairedSummary {
    /// The summary of `iterations`, bootstrapped under [`BOOTSTRAP_SEED`]; they must not be
    /// empty.
    pub fn of(iterations: &[Iteration]) -> Self {
        let differences: Vec<Micros> = iterations.iter().map(Iteration::difference).collect();
        Self {
            median: median(&differences),
            interval: bootstrap_interval(&differences, BOOTSTRAP_SEED),
        }
    }
}

/// The median of `values`, the mean of the middle two when there is an even number; they must
/// not be empty.
pub fn median(values: &[Micros]) -> Micros {
    let mut numbers: Vec<f64> = values.iter().map(|value| value.0).collect();
    Micros(median_of(&mut numbers))
}

/// The median of `numbers`, sorting them in place.
fn median_of(numbers: &mut [f64]) -> f64 {
    numbers.sort_unstable_by(f64::total_cmp);
    let middle = numbers.len() / 2;
    if !numbers.len().is_multiple_of(2) {
        return numbers[middle];
    }
    (numbers[middle - 1] + numbers[middle]) / 2.0
}

/// The nearest-rank percentile, `tenths` tenths of a percent in, of `sorted`.
fn percentile(sorted: &[f64], tenths: usize) -> f64 {
    let rank = (sorted.len() * tenths).div_ceil(1000).max(1);
    sorted[rank - 1]
}

/// The bootstrap 95% interval of the median of `differences`: [`BOOTSTRAP_RESAMPLES`]
/// resamples with replacement, drawn by an [`StdRng`] seeded with `seed`, their medians
/// sorted, the 2.5th and the 97.5th percentiles. `differences` must not be empty.
pub fn bootstrap_interval(differences: &[Micros], seed: u64) -> Interval {
    let numbers: Vec<f64> = differences.iter().map(|value| value.0).collect();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut resample = vec![0.0_f64; numbers.len()];
    let mut medians = Vec::with_capacity(BOOTSTRAP_RESAMPLES);
    for _ in 0..BOOTSTRAP_RESAMPLES {
        for slot in &mut resample {
            *slot = numbers[rng.gen_range(0..numbers.len())];
        }
        medians.push(median_of(&mut resample));
    }
    medians.sort_unstable_by(f64::total_cmp);
    Interval {
        low: Micros(percentile(&medians, LOWER_TENTHS)),
        high: Micros(percentile(&medians, UPPER_TENTHS)),
    }
}

#[cfg(test)]
mod tests;
