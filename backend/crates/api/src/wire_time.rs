//! The contract's wire instant: `google.protobuf.Timestamp` serialized as
//! **canonical** ProtoJSON (`contract/VERSIONING.md` §7.3/§7.7) — RFC 3339,
//! **Z-normalized**, 0/3/6/9 fractional digits — with the protobuf `Timestamp`
//! value range (years 0001–9999) enforced on BOTH directions of the boundary.
//!
//! Exists because `pbjson_types::Timestamp`'s `Serialize` emits `+00:00`
//! (chrono's `to_rfc3339`, `use_z = false`), which is valid RFC 3339 but not
//! canonical ProtoJSON — the spec's "generated output will always be
//! Z-normalized" — and because neither pbjson nor chrono rejects instants
//! outside 0001–9999, which the reference ProtoJSON parsers (protobuf-es
//! included) refuse: an out-of-range value accepted here would later produce a
//! response no client generated from the contract can decode. `contract-gen`
//! maps `.google.protobuf.Timestamp` to this type via `extern_path`, so every
//! generated message carries it; the golden wire test pins the `Z` encoding.

use std::ops::RangeInclusive;

use chrono::{Datelike, SecondsFormat};
use domain::datetime::DateTimeUtc;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// `google.protobuf.Timestamp`'s value range, by year: RFC 3339 instants from
/// `0001-01-01T00:00:00Z` to `9999-12-31T23:59:59.999999999Z`. Narrower than
/// chrono (±262143) and Postgres (4713 BC–294276 AD), so it is the binding
/// constraint at the wire boundary.
const WIRE_YEARS: RangeInclusive<i32> = 1..=9999;

/// The wire form of an instant — field-compatible with
/// `google.protobuf.Timestamp` (same tags, same prost shape), differing only
/// in its serde: canonical-ProtoJSON output, range-validated input.
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
    /// outside [`WIRE_YEARS`]).
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
        // The RFC 3339 grammar is pbjson's (the shared generated-parser rule);
        // this adds only the protobuf value-range check on top.
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
    use super::*;

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
