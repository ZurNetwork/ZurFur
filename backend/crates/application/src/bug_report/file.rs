use domain::{
    datetime::DateTimeUtc,
    elements::{
        bug_report::{
            BrowserText, BugDescription, BugReportId, BugReportReference, BuildId, DisplaySettings,
            ErrorStatus, NewBugReport, PageAddress, PageKind, WindowSize,
        },
        user::UserId,
    },
    ports::Unit,
};
use shared::settings::{BUG_REPORT_LIMIT, BUG_REPORT_WINDOW};

use crate::{
    bug_report::{BugReportError, BugReportResult, BugReports},
    use_case,
};

/// What the reporter sent, every item checked. Each item but the description is
/// `None` when the reporter didn't send it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportBody {
    pub description: BugDescription,
    pub page: Option<PageAddress>,
    pub page_kind: Option<PageKind>,
    pub web_version: Option<BuildId>,
    pub browser: Option<BrowserText>,
    pub window: Option<WindowSize>,
    pub display: Option<DisplaySettings>,
    pub error_status: Option<ErrorStatus>,
}

/// File `report` as `reporter`, the signed-in User.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub reporter: UserId,
    pub report: ReportBody,
}

/// The receipt: the reference to quote, and when the report was received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub reference: BugReportReference,
    pub received_at: DateTimeUtc,
}

impl BugReports<'_> {
    /// Files the report, received at `now` by the API build `api_version`. Counts inside
    /// the filing unit with the reporter locked, so concurrent filings can't overshoot;
    /// refused, the unit rolled back, once the reporter has filed the limit within the window.
    #[use_case]
    pub async fn file(
        &self,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        api_version: &BuildId,
        now: DateTimeUtc,
    ) -> BugReportResult<Output> {
        let Command { reporter, report } = cmd;
        let filed_recently = uow
            .bug_reports()
            .lock_reporter_and_count_since(&reporter, now - BUG_REPORT_WINDOW)
            .await?;
        if filed_recently >= BUG_REPORT_LIMIT {
            return Err(BugReportError::RateLimited);
        }

        let ReportBody {
            description,
            page,
            page_kind,
            web_version,
            browser,
            window,
            display,
            error_status,
        } = report;
        let id = BugReportId::generate();
        let new_report = NewBugReport {
            id,
            reporter,
            description,
            page,
            page_kind,
            web_version,
            browser,
            window,
            display,
            error_status,
            api_version: api_version.clone(),
            received_at: now,
        };

        uow.bug_reports().file(&new_report).await?;

        let reference = BugReportReference::from(&id);
        Ok(Output {
            reference,
            received_at: now,
        })
    }
}

#[cfg(test)]
mod tests;
