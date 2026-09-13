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
mod tests;
