use chrono::TimeZone;

use super::*;

/// `WireTimestamp::from(DateTimeUtc)` — the bridge every response type
/// carrying an instant goes through (e.g. `ChangelogEntryBody.created_at`)
/// — emits the exact same string chrono's own `DateTime<Utc>` serde
/// produced for every in-range instant: whole-second and fractional.
/// This is the wire-compatibility claim those callers' doc comments rest on.
#[test]
fn wire_timestamp_matches_chronos_prior_serialization() {
    let whole = chrono::Utc.with_ymd_and_hms(2025, 7, 25, 12, 0, 0).unwrap();
    assert_eq!(
        serde_json::to_string(&WireTimestamp::from(whole)).unwrap(),
        serde_json::to_string(&whole).unwrap(),
        "a whole-second instant"
    );

    let fractional = whole + chrono::Duration::microseconds(123_456);
    assert_eq!(
        serde_json::to_string(&WireTimestamp::from(fractional)).unwrap(),
        serde_json::to_string(&fractional).unwrap(),
        "a fractional instant"
    );
}

/// Canonical output: Z suffix, never `+00:00`; AutoSi fractional digits.
#[test]
fn serializes_z_normalized() {
    let whole = WireTimestamp {
        seconds: 1_753_444_800,
        nanos: 0,
    };
    assert_eq!(
        serde_json::to_string(&whole).expect("serializes"),
        "\"2025-07-25T12:00:00Z\""
    );
    let fractional = WireTimestamp {
        seconds: 1_753_444_800,
        nanos: 123_456_000,
    };
    assert_eq!(
        serde_json::to_string(&fractional).expect("serializes"),
        "\"2025-07-25T12:00:00.123456Z\""
    );
}

/// The protobuf range binds both directions: year 10000 refuses to parse
/// AND refuses to emit (a stored out-of-range value is a loud server
/// error, not a body generated clients choke on).
#[test]
fn rejects_out_of_range_years_both_ways() {
    let parse = serde_json::from_str::<WireTimestamp>("\"+10000-01-01T00:00:00Z\"");
    assert!(parse.is_err(), "year 10000 must not parse");

    let year_zero = serde_json::from_str::<WireTimestamp>("\"0000-01-01T00:00:00Z\"");
    assert!(year_zero.is_err(), "year 0 must not parse");

    let out_of_range = WireTimestamp {
        seconds: 253_402_300_800, // 10000-01-01T00:00:00Z
        nanos: 0,
    };
    assert!(
        serde_json::to_string(&out_of_range).is_err(),
        "year 10000 must not emit"
    );
}

/// ProtoJSON parsers accept any RFC 3339 offset on input; only the OUTPUT
/// is Z-normalized. The lax-in/strict-out asymmetry is the spec's.
#[test]
fn accepts_offset_input_emits_z() {
    let parsed = serde_json::from_str::<WireTimestamp>("\"2025-07-25T12:00:00+00:00\"")
        .expect("offset input parses");
    assert_eq!(
        serde_json::to_string(&parsed).expect("serializes"),
        "\"2025-07-25T12:00:00Z\""
    );
}
