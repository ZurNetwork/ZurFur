use kernel::datetime::DateTimeUtc;

/// A commission's deadline at the store's precision, so the deadline a rule compares and an
/// entry records is the one the store holds. Read back through `DateTimeUtc::from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Into)]
pub struct CommissionDeadline(DateTimeUtc);

/// The latest whole microsecond chrono holds.
const LAST_WHOLE_MICROSECOND: DateTimeUtc =
    match DateTimeUtc::from_timestamp_micros(DateTimeUtc::MAX_UTC.timestamp_micros()) {
        Some(last) => last,
        None => panic!("chrono's latest instant has a whole microsecond"),
    };

impl From<DateTimeUtc> for CommissionDeadline {
    /// Truncates `instant` to whole microseconds since the epoch, toward the past, as the store
    /// counts them: a value already whole is kept as is, and a leap second counts as the second
    /// after it. Past chrono's last day it saturates at its latest whole microsecond.
    fn from(instant: DateTimeUtc) -> Self {
        let micros = instant.timestamp_micros();
        let whole = DateTimeUtc::from_timestamp_micros(micros).unwrap_or(LAST_WHOLE_MICROSECOND);
        Self(whole)
    }
}

#[cfg(test)]
mod tests;
