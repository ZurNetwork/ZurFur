//! The contract's wire instant: `google.protobuf.Timestamp` as canonical
//! ProtoJSON — RFC 3339, Z-normalized, 0/3/6/9 fractional digits — with the
//! protobuf value range (years 0001–9999) enforced on both directions.
//! `contract-gen` maps `.google.protobuf.Timestamp` to this type via `extern_path`.

use std::ops::RangeInclusive;

use chrono::{Datelike, SecondsFormat};
use domain::datetime::DateTimeUtc;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `google.protobuf.Timestamp`'s value range, by year — narrower than chrono
/// and Postgres, so it's the binding constraint at the wire boundary.
const WIRE_YEARS: RangeInclusive<i32> = 1..=9999;

/// The wire form of an instant, field-compatible with
/// `google.protobuf.Timestamp`; differs only in serde (canonical-ProtoJSON
/// output, range-validated input).
#[derive(Clone, Copy, PartialEq, Eq, Hash, ::prost::Message)]
pub struct WireTimestamp {
    /// Seconds of UTC time since the Unix epoch.
    #[prost(int64, tag = "1")]
    pub seconds: i64,
    /// Non-negative sub-second nanoseconds.
    #[prost(int32, tag = "2")]
    pub nanos: i32,
}

impl WireTimestamp {
    /// The instant as domain time, or `None` when the pair does not name a
    /// representable in-range instant (negative/overflowing nanos, or a year
    /// outside `WIRE_YEARS`).
    pub fn as_datetime(&self) -> Option<DateTimeUtc> {
        let nanos = u32::try_from(self.nanos).ok()?;
        let datetime = chrono::DateTime::from_timestamp(self.seconds, nanos)?;
        WIRE_YEARS.contains(&datetime.year()).then_some(datetime)
    }
}

impl From<DateTimeUtc> for WireTimestamp {
    fn from(at: DateTimeUtc) -> Self {
        WireTimestamp {
            seconds: at.timestamp(),
            nanos: at.timestamp_subsec_nanos() as i32,
        }
    }
}

/// A [`WireTimestamp`] that names no instant `google.protobuf.Timestamp` can
/// carry — negative or overflowing nanos, or a year outside `WIRE_YEARS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfWireRange;

impl std::fmt::Display for OutOfWireRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the timestamp is outside the representable range")
    }
}

impl std::error::Error for OutOfWireRange {}

impl TryFrom<WireTimestamp> for DateTimeUtc {
    type Error = OutOfWireRange;

    fn try_from(value: WireTimestamp) -> Result<Self, Self::Error> {
        value.as_datetime().ok_or(OutOfWireRange)
    }
}

impl Serialize for WireTimestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let datetime = self.as_datetime().ok_or_else(|| {
            serde::ser::Error::custom(
                "timestamp outside google.protobuf.Timestamp's 0001-9999 range — \
                 refusing to emit a value generated clients cannot decode",
            )
        })?;
        let canonical = datetime.to_rfc3339_opts(SecondsFormat::AutoSi, true);
        serializer.serialize_str(&canonical)
    }
}

impl<'de> Deserialize<'de> for WireTimestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Grammar is pbjson's; this adds only the protobuf value-range check.
        let parsed = pbjson_types::Timestamp::deserialize(deserializer)?;
        let timestamp = WireTimestamp {
            seconds: parsed.seconds,
            nanos: parsed.nanos,
        };
        let _ = timestamp.as_datetime().ok_or_else(|| {
            serde::de::Error::custom(
                "timestamp must be between 0001-01-01T00:00:00Z and \
                 9999-12-31T23:59:59Z inclusive (google.protobuf.Timestamp)",
            )
        })?;
        Ok(timestamp)
    }
}

#[cfg(test)]
mod tests {
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
}
