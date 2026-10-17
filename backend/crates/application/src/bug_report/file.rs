use domain::{
    datetime::DateTimeUtc,
    elements::{
        bug_report::{
            BrowserText, BugDescription, BugReportId, BugReportReference, BuildId, DisplaySettings,
            ErrorStatus, NewBugReport, PageAddress, PageKind, WindowSize,
        },
        user::UserId,
    },
};
use shared::settings::{BUG_REPORT_LIMIT, BUG_REPORT_WINDOW};
use uuid::Uuid;

use crate::{
    LazyUnit, Ports,
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
    /// Files the report, received at `now` by the API build `api_version`. Refused,
    /// opening no unit, once the reporter has filed the limit within the window.
    #[use_case]
    pub async fn file(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        api_version: &BuildId,
        now: DateTimeUtc,
    ) -> BugReportResult<Output> {
        let Command { reporter, report } = cmd;
        let filed_recently = ports
            .bug_reports
            .count_filed_since(&reporter, now - BUG_REPORT_WINDOW)
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
        let id = BugReportId::from(Uuid::now_v7());
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

        let uow = uow.open().await?;
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
