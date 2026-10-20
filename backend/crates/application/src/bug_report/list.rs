use domain::{datetime::DateTimeUtc, elements::bug_report::StoredBugReport};

use crate::{
    Ports,
    bug_report::{BugReportResult, BugReports},
    use_case,
};

/// The reports received at or after `since`; every report when `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Query {
    pub since: Option<DateTimeUtc>,
}

/// The reports, oldest first, their texts as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub reports: Vec<StoredBugReport>,
}

impl BugReports<'_> {
    /// The reports the query selects, ordered by received time, then id.
    #[use_case]
    pub async fn list(&self, #[ports] ports: &Ports, query: Query) -> BugReportResult<Output> {
        let reports = ports.bug_reports.list_since(query.since).await?;
        Ok(Output { reports })
    }
}

#[cfg(test)]
mod tests;
