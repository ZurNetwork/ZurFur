//! In-memory fakes for the bug-report ports: an append-only log, staged like
//! every other private-store write, and one filing lock standing in for pg's
//! reporter row lock.

use std::sync::Arc;

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::bug_report::{NewBugReport, StoredBugReport};
use domain::elements::text::StoredText;
use domain::elements::user::UserId;
use domain::ports::{BugReportStore, BugReportWrites};

use crate::MemBackend;

/// In-memory [`BugReportStore`] read surface over the shared [`MemBackend`].
pub struct MemBugReportStore(pub(crate) MemBackend);

#[async_trait]
impl BugReportStore for MemBugReportStore {
    /// Scan the log, then sort by received time and id, as the pg read orders.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn list_since(&self, since: Option<DateTimeUtc>) -> anyhow::Result<Vec<StoredBugReport>> {
        let reports = self
            .0
            .bug_reports
            .lock()
            .expect("MemBackend bug_reports mutex poisoned");
        let mut listed: Vec<&NewBugReport> = reports
            .iter()
            .filter(|report| since.is_none_or(|since| report.received_at >= since))
            .collect();
        listed.sort_by_key(|report| (report.received_at, *report.id.as_ref()));
        let stored = listed.into_iter().map(stored_report).collect();
        Ok(stored)
    }
}

/// In-memory [`BugReportWrites`] view, vended by
/// [`MemUnitOfWork::bug_reports`](crate::MemUnitOfWork) over the unit's staging
/// snapshot, so a report reaches the shared log only on commit.
pub struct MemBugReportWrites<'a> {
    /// The shared backend: the committed log and the filing lock.
    pub(crate) shared: MemBackend,
    /// The unit's staging snapshot, where filing writes.
    pub(crate) staged: MemBackend,
    /// The unit's slot for the filing lock, so the lock lives as long as the unit.
    pub(crate) filing_lock: &'a mut Option<tokio::sync::OwnedMutexGuard<()>>,
}

#[async_trait]
impl BugReportWrites for MemBugReportWrites<'_> {
    /// Take the filing lock unless this unit holds it, then count the committed
    /// log plus this unit's own staged reports, not the snapshot taken at begin.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn lock_reporter_and_count_since(
        &mut self,
        reporter: &UserId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        if self.filing_lock.is_none() {
            let filing = Arc::clone(&self.shared.bug_report_filing);
            *self.filing_lock = Some(filing.lock_owned().await);
        }
        let committed = self
            .shared
            .bug_reports
            .lock()
            .expect("MemBackend bug_reports mutex poisoned");
        let staged = self
            .staged
            .bug_reports
            .lock()
            .expect("MemBackend bug_reports mutex poisoned");
        let own = staged
            .iter()
            .filter(|report| !committed.iter().any(|filed| filed.id == report.id));
        let filed = committed
            .iter()
            .chain(own)
            .filter(|report| &report.reporter == reporter)
            .filter(|report| report.received_at >= since)
            .count();
        let filed = i64::try_from(filed)?;
        Ok(filed)
    }

    /// Append the report. Refuses a reporter who is not a User, as the pg
    /// foreign key onto `users` does, and an id already filed, as its key does.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn file(&mut self, report: &NewBugReport) -> anyhow::Result<()> {
        let reporter_is_a_user = self
            .staged
            .users
            .lock()
            .expect("MemBackend users mutex poisoned")
            .contains_key(report.reporter.did());
        anyhow::ensure!(reporter_is_a_user, "a bug report's reporter is not a User");
        let mut reports = self
            .staged
            .bug_reports
            .lock()
            .expect("MemBackend bug_reports mutex poisoned");
        let already_filed = reports.iter().any(|filed| filed.id == report.id);
        anyhow::ensure!(!already_filed, "a bug report with this id is already filed");
        reports.push(report.clone());
        Ok(())
    }
}

/// A filed report as the store reads it back, its texts as stored.
fn stored_report(report: &NewBugReport) -> StoredBugReport {
    let stored_text = |text: &str| StoredText::from(text.to_owned());
    StoredBugReport {
        id: report.id,
        reporter: report.reporter.clone(),
        description: stored_text(report.description.as_ref()),
        page: report.page.as_ref().map(|page| stored_text(page.as_ref())),
        page_kind: report
            .page_kind
            .as_ref()
            .map(|kind| stored_text(kind.as_ref())),
        web_version: report
            .web_version
            .as_ref()
            .map(|version| stored_text(version.as_ref())),
        browser: report
            .browser
            .as_ref()
            .map(|browser| stored_text(browser.as_ref())),
        window: report.window,
        display: report.display,
        error_status: report.error_status,
        api_version: stored_text(report.api_version.as_ref()),
        received_at: report.received_at,
    }
}

#[cfg(test)]
mod tests;
