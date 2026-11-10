use super::*;

/// An instant written as RFC 3339.
fn instant(text: &str) -> DateTimeUtc {
    text.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

/// The instant `deadline` holds.
fn held(deadline: CommissionDeadline) -> DateTimeUtc {
    DateTimeUtc::from(deadline)
}

// Sub-microsecond nanoseconds are dropped toward the past: after 2000, between 1970 and 2000,
// and before 1970, where the instant's offset from either epoch is negative.
#[test]
fn a_deadline_drops_its_sub_microsecond_nanoseconds() {
    let cases = [
        ("2099-03-01T00:00:00.000000001Z", "2099-03-01T00:00:00Z"),
        (
            "2099-03-01T00:00:00.123456999Z",
            "2099-03-01T00:00:00.123456Z",
        ),
        (
            "1999-12-31T23:59:59.999999999Z",
            "1999-12-31T23:59:59.999999Z",
        ),
        ("1985-06-15T08:30:00.000000500Z", "1985-06-15T08:30:00Z"),
        (
            "1969-07-20T20:17:40.123456001Z",
            "1969-07-20T20:17:40.123456Z",
        ),
        (
            "1969-12-31T23:59:59.999999999Z",
            "1969-12-31T23:59:59.999999Z",
        ),
    ];

    for (finer, whole) in cases {
        let deadline = CommissionDeadline::from(instant(finer));

        assert_eq!(held(deadline), instant(whole), "{finer}");
    }
}

// A whole-microsecond instant is kept as it is, before and after either epoch.
#[test]
fn a_whole_microsecond_deadline_is_kept() {
    let whole = [
        "2099-03-01T00:00:00Z",
        "2000-01-01T00:00:00.000001Z",
        "2000-01-01T00:00:00Z",
        "1999-12-31T23:59:59.999999Z",
        "1970-01-01T00:00:00Z",
        "1969-07-20T20:17:40.123456Z",
        "0001-01-01T00:00:00.000001Z",
    ];

    for text in whole {
        let deadline = CommissionDeadline::from(instant(text));

        assert_eq!(held(deadline), instant(text), "{text}");
    }
}

// A leap second counts as the second after it, as the store counts it.
#[test]
fn a_leap_second_counts_as_the_second_after_it() {
    let leap = instant("2016-12-31T23:59:60.5000005Z");

    let deadline = CommissionDeadline::from(leap);

    assert_eq!(held(deadline), instant("2017-01-01T00:00:00.500000Z"));
}

// The latest instant chrono holds keeps its whole microseconds, and a leap second on its last
// day, which counts past anything chrono holds, saturates there.
#[test]
fn the_latest_instants_saturate_at_the_last_whole_microsecond() {
    let last_leap = chrono::NaiveDate::MAX
        .and_hms_nano_opt(23, 59, 59, 1_500_000_000)
        .expect("a leap second on chrono's last day")
        .and_utc();

    let latest = CommissionDeadline::from(DateTimeUtc::MAX_UTC);
    let leaping = CommissionDeadline::from(last_leap);

    assert_eq!(held(latest), LAST_WHOLE_MICROSECOND);
    assert_eq!(held(leaping), LAST_WHOLE_MICROSECOND);
    assert_eq!(LAST_WHOLE_MICROSECOND.timestamp_subsec_nanos() % 1_000, 0);
}
