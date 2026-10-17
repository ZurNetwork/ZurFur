//! `POST /bug-reports`: the signed-in visitor files a bug report. The reporter
//! comes from the session alone, and no answer repeats what was sent: a body
//! that doesn't decode gets one fixed sentence, a refused item one fixed
//! sentence per item. No route reads reports back.

use application::bug_report::{
    BugReportError,
    file::{self, ReportBody},
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::Utc;
use domain::elements::bug_report::{
    BugDescription, DisplaySettings, ErrorStatus, WindowSide, WindowSideError, WindowSize,
};

use crate::{
    AppState,
    extract::CallingUser,
    generated::{FileBugReportRequest, FileBugReportResponse, file_bug_report_request},
    problem::Problem,
    wire_time::WireTimestamp,
};

/// The answer to a body that doesn't decode: an unknown key, a wrong type, not JSON.
const UNREADABLE_BODY: &str = "The request body isn't a bug report this endpoint can read.";
/// The answer to a refused description.
const DESCRIPTION_REFUSED: &str = "The description must be 1 to 4000 characters, with no control \
                                   characters other than newlines and tabs.";
/// The answer to a refused page.
const PAGE_REFUSED: &str = "The page must be an app path starting with `/`, one line of at most \
                            2048 bytes, not starting with `//`, with no query, no fragment and no \
                            backslash.";
/// The answer to a refused page kind.
const PAGE_KIND_REFUSED: &str = "The page kind must be one line of at most 64 bytes.";
/// The answer to a refused web version.
const WEB_VERSION_REFUSED: &str = "The web version must be one line of at most 64 bytes.";
/// The answer to a refused browser.
const BROWSER_REFUSED: &str = "The browser must be one line of at most 512 bytes.";
/// The answer to a refused window size.
const WINDOW_REFUSED: &str = "The window's width and height must each be 1 to 100000 pixels.";
/// The answer to a refused error status.
const ERROR_STATUS_REFUSED: &str = "The error status must be an HTTP error status, 400 to 599.";
/// The answer once the reporter has filed as many reports as the window allows.
const RATE_LIMITED: &str = "Too many bug reports recently. Please wait before sending another.";

/// The bug-report route, for the cookie surface.
pub(crate) fn bug_reports_router() -> Router<AppState> {
    Router::new().route("/bug-reports", post(file_bug_report))
}

impl From<BugReportError> for Problem {
    /// The rate limit is a `429`; a store failure the fixed `500`.
    fn from(err: BugReportError) -> Self {
        match err {
            BugReportError::RateLimited => Problem::rate_limited(RATE_LIMITED),
            BugReportError::Infrastructure(err) => Problem::from(err),
        }
    }
}

impl TryFrom<FileBugReportRequest> for ReportBody {
    type Error = Problem;

    /// Checks each item, refusing the first bad one with its field's fixed sentence.
    fn try_from(request: FileBugReportRequest) -> Result<Self, Self::Error> {
        let FileBugReportRequest {
            description,
            page,
            page_kind,
            web_version,
            browser,
            window,
            display,
            error_status,
        } = request;
        let description = BugDescription::try_from(description)
            .map_err(|_| Problem::invalid_request(DESCRIPTION_REFUSED))?;
        let page = checked(page, PAGE_REFUSED)?;
        let page_kind = checked(page_kind, PAGE_KIND_REFUSED)?;
        let web_version = checked(web_version, WEB_VERSION_REFUSED)?;
        let browser = checked(browser, BROWSER_REFUSED)?;
        let window = window
            .map(WindowSize::try_from)
            .transpose()
            .map_err(|_| Problem::invalid_request(WINDOW_REFUSED))?;
        let display = display.map(DisplaySettings::from);
        let error_status = error_status
            .map(ErrorStatus::try_from)
            .transpose()
            .map_err(|_| Problem::invalid_request(ERROR_STATUS_REFUSED))?;
        let report = ReportBody {
            description,
            page,
            page_kind,
            web_version,
            browser,
            window,
            display,
            error_status,
        };
        Ok(report)
    }
}

/// `raw` checked as `T` when sent; a refusal answers `refusal`, never the text.
fn checked<T: TryFrom<String>>(
    raw: Option<String>,
    refusal: &'static str,
) -> Result<Option<T>, Problem> {
    raw.map(T::try_from)
        .transpose()
        .map_err(|_| Problem::invalid_request(refusal))
}

impl TryFrom<file_bug_report_request::Window> for WindowSize {
    type Error = WindowSideError;

    /// Both sides checked.
    fn try_from(window: file_bug_report_request::Window) -> Result<Self, Self::Error> {
        let width = WindowSide::try_from(window.width)?;
        let height = WindowSide::try_from(window.height)?;
        Ok(WindowSize { width, height })
    }
}

impl From<file_bug_report_request::Display> for DisplaySettings {
    /// The three preferences as sent; an empty object is all three off.
    fn from(display: file_bug_report_request::Display) -> Self {
        DisplaySettings {
            reduced_motion: display.reduced_motion,
            more_contrast: display.more_contrast,
            forced_colors: display.forced_colors,
        }
    }
}

impl From<file::Output> for FileBugReportResponse {
    /// The receipt on the wire.
    fn from(receipt: file::Output) -> Self {
        FileBugReportResponse {
            reference: receipt.reference.to_string(),
            received_at: Some(WireTimestamp::from(receipt.received_at)),
        }
    }
}

/// `POST /bug-reports` — files the report as the signed-in visitor.
///
/// - `201 Created` with the receipt
/// - `401` without a session; `422` for a body that doesn't decode or a refused item;
///   `429` past the rate limit
async fn file_bug_report(
    State(state): State<AppState>,
    CallingUser(reporter): CallingUser,
    body: Result<Json<FileBugReportRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request(UNREADABLE_BODY))?;
    let report = ReportBody::try_from(body)?;
    let command = file::Command { reporter, report };

    let receipt = state
        .app()
        .bug_reports()
        .file(command, &state.config.build_id, Utc::now())
        .await?;

    let body = FileBugReportResponse::from(receipt);
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
