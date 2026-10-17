use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
use crate::elements::bug_report::{NewBugReport, StoredBugReport};
use crate::elements::user::UserId;

/// The read surface of the bug reports Users file — pool-backed and
/// non-transactional; filing and the rate limit's count live on [`BugReportWrites`].
#[async_trait]
pub trait BugReportStore: Send + Sync {
    /// Every report received at or after `since` (every report when `None`),
    /// ordered by received time, then id.
    async fn list_since(&self, since: Option<DateTimeUtc>) -> anyhow::Result<Vec<StoredBugReport>>;
}

/// The write surface of bug reports — reachable only on an open
/// [`UnitOfWork`](super::UnitOfWork).
#[async_trait]
pub trait BugReportWrites: Send {
    /// Lock `reporter`'s User row until the unit ends, then count their reports
    /// received at or after `since` (one received exactly at `since` counts). A
    /// second unit locking the same reporter waits, then counts what this one filed.
    async fn lock_reporter_and_count_since(
        &mut self,
        reporter: &UserId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64>;

    /// Store one report. Refused when its reporter is not a User.
    async fn file(&mut self, report: &NewBugReport) -> anyhow::Result<()>;
}
