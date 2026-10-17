//! Bug reports — what a signed-in User files from the web app when something
//! goes wrong: their description, the page and browser it went wrong in, and
//! the API build that received it.
//!
//! Every text value is trimmed, never empty, and refuses control characters
//! and the explicit bidirectional formatting characters; the description alone
//! keeps newlines and tabs. Other invisible characters are left in, for a
//! printer to neutralize.

mod errors;
mod id;
mod rows;
mod values;

pub use errors::{
    BrowserTextError, BugDescriptionError, BuildIdError, ErrorStatusError, PageAddressError,
    PageKindError, WindowSideError,
};
pub use id::{BugReportId, BugReportReference};
pub use rows::{NewBugReport, StoredBugReport};
pub use values::{
    BrowserText, BugDescription, BuildId, DisplaySettings, ErrorStatus, PageAddress, PageKind,
    WindowSide, WindowSize,
};
