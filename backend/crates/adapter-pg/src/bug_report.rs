//! [`BugReportStore`] (reads) and [`BugReportWrites`] (filing) over
//! PostgreSQL, in the `bug_report` table. Reads are pool-backed; filing is a
//! write, reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.bug_reports()`).

use domain::datetime::DateTimeUtc;
use domain::elements::bug_report::{
    BugReportId, DisplaySettings, ErrorStatus, NewBugReport, StoredBugReport, WindowSide,
    WindowSize,
};
use domain::elements::did::Did;
use domain::elements::text::StoredText;
use domain::elements::user::UserId;
use domain::ports::{BugReportStore, BugReportWrites};
use sqlx::{PgConnection, PgPool};

use crate::queries::bug_report as sql;

/// PostgreSQL read store for bug reports (the [`BugReportStore`] surface).
/// Filing lives on [`PgBugReportWrites`].
pub struct PgBugReportStore {
    pool: PgPool,
}

impl PgBugReportStore {
    /// Wraps a [`PgPool`] as a [`BugReportStore`]. Clones the pool handle (an
    /// `Arc`), leaving the caller's intact.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// PostgreSQL write view for filing bug reports (the [`BugReportWrites`]
/// surface), over an open transaction. Built by `uow.bug_reports()`.
pub struct PgBugReportWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl BugReportStore for PgBugReportStore {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn count_filed_since(
        &self,
        reporter: &UserId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        let filed = sql::count_filed_since(&self.pool, reporter.as_ref(), since).await?;
        Ok(filed)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn list_since(&self, since: Option<DateTimeUtc>) -> anyhow::Result<Vec<StoredBugReport>> {
        let rows = sql::list_since(&self.pool, since).await?;
        rows.into_iter().map(StoredBugReport::try_from).collect()
    }
}

#[async_trait::async_trait]
impl BugReportWrites for PgBugReportWrites<'_> {
    /// Inserts the report. The `users` foreign key refuses a reporter who is
    /// not a User.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn file(&mut self, report: &NewBugReport) -> anyhow::Result<()> {
        let window_width = report.window.map(|window| side_column(window.width));
        let window_height = report.window.map(|window| side_column(window.height));
        let error_status = report
            .error_status
            .map(|status| i32::from(u16::from(status)));
        sql::file(
            &mut *self.conn,
            *report.id.as_ref(),
            report.reporter.as_ref(),
            report.description.as_ref(),
            report.page.as_ref().map(AsRef::as_ref),
            report.page_kind.as_ref().map(AsRef::as_ref),
            report.web_version.as_ref().map(AsRef::as_ref),
            report.browser.as_ref().map(AsRef::as_ref),
            window_width,
            window_height,
            report.display.map(|display| display.reduced_motion),
            report.display.map(|display| display.more_contrast),
            report.display.map(|display| display.forced_colors),
            error_status,
            report.api_version.as_ref(),
            report.received_at,
        )
        .await?;
        Ok(())
    }
}

/// A window side as its `integer` column holds it. Every side fits: the
/// widest is 100,000.
fn side_column(side: WindowSide) -> i32 {
    i32::try_from(u32::from(side)).expect("a window side is at most 100,000")
}

impl TryFrom<sql::BugReportRow> for StoredBugReport {
    type Error = anyhow::Error;

    /// Loads a stored row. The table's checks keep every number in range and
    /// each tick box whole, so a refusal here means the row was corrupted.
    fn try_from(row: sql::BugReportRow) -> anyhow::Result<Self> {
        let window = match (row.window_width, row.window_height) {
            (Some(width), Some(height)) => Some(WindowSize {
                width: WindowSide::try_from(width)?,
                height: WindowSide::try_from(height)?,
            }),
            (None, None) => None,
            _ => anyhow::bail!("a stored bug report holds half a window size"),
        };
        let display = match (row.reduced_motion, row.more_contrast, row.forced_colors) {
            (Some(reduced_motion), Some(more_contrast), Some(forced_colors)) => {
                Some(DisplaySettings {
                    reduced_motion,
                    more_contrast,
                    forced_colors,
                })
            }
            (None, None, None) => None,
            _ => anyhow::bail!("a stored bug report holds part of its display settings"),
        };
        let error_status = row.error_status.map(ErrorStatus::try_from).transpose()?;
        let report = Self {
            id: BugReportId::from(row.id),
            reporter: UserId::from(Did::from(row.reporter_id)),
            description: StoredText::from(row.description),
            page: row.page.map(StoredText::from),
            page_kind: row.page_kind.map(StoredText::from),
            web_version: row.web_version.map(StoredText::from),
            browser: row.browser.map(StoredText::from),
            window,
            display,
            error_status,
            api_version: StoredText::from(row.api_version),
            received_at: row.received_at,
        };
        Ok(report)
    }
}
