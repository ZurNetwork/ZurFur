use super::{
    BrowserText, BugDescription, BugReportId, BuildId, DisplaySettings, ErrorStatus, PageAddress,
    PageKind, WindowSize,
};
use crate::{
    datetime::DateTimeUtc,
    elements::{text::StoredText, user::UserId},
};

/// A bug report as the store writes it. Every text is a checked value, so no
/// unchecked text can reach the write. Each optional item is one tick box,
/// `None` when the reporter didn't send it.
///
/// ```compile_fail,E0308
/// use domain::elements::{bug_report::NewBugReport, text::StoredText};
///
/// fn store_unchecked(new: &mut NewBugReport) {
///     new.description = StoredText::from("   ".to_owned()); // refused: not a BugDescription
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBugReport {
    /// The app-private id (UUIDv7, so it sorts by filing time).
    pub id: BugReportId,
    /// The User who filed it.
    pub reporter: UserId,
    /// What went wrong.
    pub description: BugDescription,
    /// The app path it was filed from, or `None`.
    pub page: Option<PageAddress>,
    /// The kind of page it was filed from, or `None`.
    pub page_kind: Option<PageKind>,
    /// The web app's build, or `None`.
    pub web_version: Option<BuildId>,
    /// The browser's user agent, or `None`.
    pub browser: Option<BrowserText>,
    /// The browser window's size, or `None` when not sent.
    pub window: Option<WindowSize>,
    /// The browser's display preferences, or `None` when not sent.
    pub display: Option<DisplaySettings>,
    /// The status of the error page it was filed from, or `None`.
    pub error_status: Option<ErrorStatus>,
    /// The API build that received it.
    pub api_version: BuildId,
    /// When the API received it.
    pub received_at: DateTimeUtc,
}

/// A bug report as the store holds it. Its texts are loaded without re-checking,
/// so a reader escapes them before printing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBugReport {
    /// The app-private id.
    pub id: BugReportId,
    /// The User who filed it.
    pub reporter: UserId,
    /// What went wrong, as stored.
    pub description: StoredText,
    /// The app path it was filed from, as stored, or `None`.
    pub page: Option<StoredText>,
    /// The kind of page it was filed from, as stored, or `None`.
    pub page_kind: Option<StoredText>,
    /// The web app's build, as stored, or `None`.
    pub web_version: Option<StoredText>,
    /// The browser's user agent, as stored, or `None`.
    pub browser: Option<StoredText>,
    /// The browser window's size, or `None` when not sent.
    pub window: Option<WindowSize>,
    /// The browser's display preferences, or `None` when not sent.
    pub display: Option<DisplaySettings>,
    /// The status of the error page it was filed from, or `None`.
    pub error_status: Option<ErrorStatus>,
    /// The API build that received it, as stored.
    pub api_version: StoredText,
    /// When the API received it.
    pub received_at: DateTimeUtc,
}
