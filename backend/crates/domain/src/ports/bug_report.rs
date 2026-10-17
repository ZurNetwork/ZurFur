use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
use crate::elements::bug_report::{NewBugReport, StoredBugReport};
use crate::elements::user::UserId;

/// The read surface of the bug reports Users file — pool-backed and
/// non-transactional; filing lives on [`BugReportWrites`].
#[async_trait]
pub trait BugReportStore: Send + Sync {
    /// How many reports `reporter` filed at or after `since`; a report
    /// received exactly at `since` counts.
    async fn count_filed_since(&self, reporter: &UserId, since: DateTimeUtc)
    -> anyhow::Result<i64>;

    /// Every report received at or after `since` (every report when `None`),
    /// ordered by received time, then id.
    async fn list_since(&self, since: Option<DateTimeUtc>) -> anyhow::Result<Vec<StoredBugReport>>;
}

/// The write surface of bug reports — reachable only on an open
/// [`UnitOfWork`](super::UnitOfWork).
#[async_trait]
pub trait BugReportWrites: Send {
    /// Store one report. Refused when its reporter is not a User.
    async fn file(&mut self, report: &NewBugReport) -> anyhow::Result<()>;
}
