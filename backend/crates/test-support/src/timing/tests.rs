use super::*;

/// `count` iterations, the made-up id's time `absent_micros(i)` and the real thing's that plus
/// `difference_micros(i)`.
fn iterations(
    count: u64,
    absent_micros: impl Fn(u64) -> u64,
    difference_micros: impl Fn(u64) -> i64,
) -> Vec<Iteration> {
    (0..count)
        .map(|run| {
            let absent = absent_micros(run);
            let not_yours = absent
                .checked_add_signed(difference_micros(run))
                .expect("a time");
            Iteration {
                absent: Duration::from_micros(absent),
                not_yours: Duration::from_micros(not_yours),
            }
        })
        .collect()
}

#[test]
fn a_constant_series_has_a_zero_width_interval_at_that_constant() {
    let runs = iterations(60, |_| 1_000, |_| 120);

    let summary = PairedSummary::of(&runs);

    let expected = Micros::from(120.0);
    assert_eq!(summary.median, expected);
    assert_eq!(summary.interval.low, expected);
    assert_eq!(summary.interval.high, expected);
}

#[test]
fn a_series_shifted_by_a_known_amount_is_recovered() {
    // The real thing is 250 µs slower, give or take two, while the base time wanders.
    let runs = iterations(
        200,
        |run| 900 + 13 * (run % 17),
        |run| 250 + i64::try_from(run % 5).expect("a small number") - 2,
    );

    let summary = PairedSummary::of(&runs);

    let shift = Micros::from(250.0);
    assert_eq!(summary.median, shift);
    assert!(summary.interval.contains(shift), "{summary:?}");
    let widest = Interval {
        low: Micros::from(248.0),
        high: Micros::from(252.0),
    };
    assert!(widest.contains(summary.interval.low), "{summary:?}");
    assert!(widest.contains(summary.interval.high), "{summary:?}");
}

#[test]
fn a_real_thing_that_answers_faster_has_a_negative_median() {
    let runs = iterations(
        40,
        |_| 2_000,
        |run| -(80 + i64::try_from(run % 3).expect("small")),
    );

    let summary = PairedSummary::of(&runs);

    assert_eq!(summary.median, Micros::from(-81.0));
    assert!(summary.interval.high < Micros::from(0.0), "{summary:?}");
}

#[test]
fn the_same_seed_gives_the_same_interval() {
    let runs = iterations(
        120,
        |run| 1_000 + (run * 37) % 101,
        |run| i64::try_from((run * 53) % 211).expect("a small number") - 105,
    );
    let differences: Vec<Micros> = runs.iter().map(Iteration::difference).collect();

    let first = bootstrap_interval(&differences, BOOTSTRAP_SEED);
    let second = bootstrap_interval(&differences, BOOTSTRAP_SEED);

    assert_eq!(first, second);
    assert!(first.low < first.high, "{first:?}");
}

#[test]
fn another_seed_may_draw_other_resamples_but_brackets_the_median_alike() {
    let runs = iterations(
        120,
        |run| 1_000 + (run * 37) % 101,
        |run| i64::try_from((run * 53) % 211).expect("a small number") - 105,
    );
    let differences: Vec<Micros> = runs.iter().map(Iteration::difference).collect();
    let observed = median(&differences);

    let other = bootstrap_interval(&differences, BOOTSTRAP_SEED + 1);

    assert!(other.contains(observed), "{other:?} against {observed:?}");
}

#[test]
fn the_median_of_an_even_count_is_the_mean_of_the_middle_two() {
    let values = [10.0, 40.0, 20.0, 30.0].map(Micros::from);

    assert_eq!(median(&values), Micros::from(25.0));
}

#[test]
fn the_median_of_an_odd_count_is_the_middle_one() {
    let values = [5.0, -3.0, 9.0].map(Micros::from);

    assert_eq!(median(&values), Micros::from(5.0));
}

#[test]
fn an_iteration_s_difference_is_signed_real_thing_minus_made_up_id() {
    let slower = Iteration {
        absent: Duration::from_micros(100),
        not_yours: Duration::from_micros(130),
    };
    let faster = Iteration {
        absent: Duration::from_micros(130),
        not_yours: Duration::from_micros(100),
    };

    assert_eq!(slower.difference(), Micros::from(30.0));
    assert_eq!(faster.difference(), Micros::from(-30.0));
}
