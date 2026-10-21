use super::{
    BrowserTextError, BugDescriptionError, BuildIdError, ErrorStatusError, PageAddressError,
    PageKindError, WindowSideError,
};
use kernel::string_builder::{StringBuilder, StringBuilderViolation};

/// Why a one-line text was refused, before each value names the refusal as its own.
enum LineViolation {
    Empty,
    TooLong,
    ControlCharacter,
}

/// Trim `raw`, then refuse it when empty, holding a control, bidirectional formatting, line
/// separator or paragraph separator character, or longer than `max_bytes`.
fn one_line(raw: String, max_bytes: usize) -> Result<String, LineViolation> {
    let text = StringBuilder::non_empty_from(raw)
        .no_control()
        .no_bidi_control()
        .no_line_separator()
        .build()
        .map_err(|violation| match violation {
            StringBuilderViolation::Empty => LineViolation::Empty,
            StringBuilderViolation::ControlCharacter => LineViolation::ControlCharacter,
            StringBuilderViolation::TooLong { .. } => {
                // Unreachable: this chain never calls max_chars; the cap is in bytes, below.
                debug_assert!(false, "one_line's chain never calls max_chars");
                LineViolation::TooLong
            }
        })?;
    if text.len() > max_bytes {
        return Err(LineViolation::TooLong);
    }
    Ok(text)
}

/// What went wrong, in the reporter's words: trimmed, never empty, at most
/// [`MAX_CHARS`](Self::MAX_CHARS) characters, holding no control character but newline and tab,
/// and no bidirectional formatting character.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct BugDescription(String);

impl BugDescription {
    /// The length cap, in characters (code points).
    pub const MAX_CHARS: usize = 4000;
}

impl TryFrom<String> for BugDescription {
    type Error = BugDescriptionError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::non_empty_from(raw)
            .max_chars(Self::MAX_CHARS)
            .no_control_except(&['\n', '\t'])
            .no_bidi_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => BugDescriptionError::Empty,
                StringBuilderViolation::TooLong { .. } => BugDescriptionError::TooLong,
                StringBuilderViolation::ControlCharacter => BugDescriptionError::ControlCharacter,
            })
    }
}

impl TryFrom<&str> for BugDescription {
    type Error = BugDescriptionError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

/// The app path a report was filed from: a one-line text starting with one `/` (never `//`,
/// which a browser reads as another host), at most [`MAX_BYTES`](Self::MAX_BYTES) bytes, with no
/// backslash (which a browser reads as `/`), no query (`?`) and no fragment (`#`).
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct PageAddress(String);

impl PageAddress {
    /// The length cap, in bytes.
    pub const MAX_BYTES: usize = 2048;
}

impl TryFrom<String> for PageAddress {
    type Error = PageAddressError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let page = one_line(raw, Self::MAX_BYTES).map_err(|violation| match violation {
            LineViolation::Empty => PageAddressError::Empty,
            LineViolation::TooLong => PageAddressError::TooLong,
            LineViolation::ControlCharacter => PageAddressError::ControlCharacter,
        })?;
        if page.contains('\\') {
            return Err(PageAddressError::Backslash);
        }
        if !page.starts_with('/') {
            return Err(PageAddressError::NotAbsolute);
        }
        if page.starts_with("//") {
            return Err(PageAddressError::ProtocolRelative);
        }
        if page.contains(['?', '#']) {
            return Err(PageAddressError::QueryOrFragment);
        }
        Ok(Self(page))
    }
}

impl TryFrom<&str> for PageAddress {
    type Error = PageAddressError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

/// The kind of page a report was filed from: a one-line text of at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct PageKind(String);

impl PageKind {
    /// The length cap, in bytes.
    pub const MAX_BYTES: usize = 64;
}

impl TryFrom<String> for PageKind {
    type Error = PageKindError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        one_line(raw, Self::MAX_BYTES)
            .map(Self)
            .map_err(|violation| match violation {
                LineViolation::Empty => PageKindError::Empty,
                LineViolation::TooLong => PageKindError::TooLong,
                LineViolation::ControlCharacter => PageKindError::ControlCharacter,
            })
    }
}

impl TryFrom<&str> for PageKind {
    type Error = PageKindError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

/// A build of the web app or the API: a one-line text of at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct BuildId(String);

impl BuildId {
    /// The length cap, in bytes.
    pub const MAX_BYTES: usize = 64;
}

impl TryFrom<String> for BuildId {
    type Error = BuildIdError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        one_line(raw, Self::MAX_BYTES)
            .map(Self)
            .map_err(|violation| match violation {
                LineViolation::Empty => BuildIdError::Empty,
                LineViolation::TooLong => BuildIdError::TooLong,
                LineViolation::ControlCharacter => BuildIdError::ControlCharacter,
            })
    }
}

impl TryFrom<&str> for BuildId {
    type Error = BuildIdError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

/// The browser a report was filed from, as its user agent names it: a one-line text of at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct BrowserText(String);

impl BrowserText {
    /// The length cap, in bytes.
    pub const MAX_BYTES: usize = 512;
}

impl TryFrom<String> for BrowserText {
    type Error = BrowserTextError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        one_line(raw, Self::MAX_BYTES)
            .map(Self)
            .map_err(|violation| match violation {
                LineViolation::Empty => BrowserTextError::Empty,
                LineViolation::TooLong => BrowserTextError::TooLong,
                LineViolation::ControlCharacter => BrowserTextError::ControlCharacter,
            })
    }
}

impl TryFrom<&str> for BrowserText {
    type Error = BrowserTextError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

/// One side of a browser window, in CSS pixels: [`MIN`](Self::MIN) to [`MAX`](Self::MAX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, derive_more::Into)]
pub struct WindowSide(u32);

impl WindowSide {
    /// The narrowest side.
    pub const MIN: u32 = 1;
    /// The widest side.
    pub const MAX: u32 = 100_000;
}

impl TryFrom<i32> for WindowSide {
    type Error = WindowSideError;

    fn try_from(raw: i32) -> Result<Self, Self::Error> {
        let side = u32::try_from(raw).map_err(|_| WindowSideError::OutOfRange)?;
        if !(Self::MIN..=Self::MAX).contains(&side) {
            return Err(WindowSideError::OutOfRange);
        }
        Ok(Self(side))
    }
}

/// The size of the browser window a report was filed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowSize {
    /// The window's width.
    pub width: WindowSide,
    /// The window's height.
    pub height: WindowSide,
}

/// The HTTP status of the error page a report was filed from: [`MIN`](Self::MIN) to
/// [`MAX`](Self::MAX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display, derive_more::Into)]
pub struct ErrorStatus(u16);

impl ErrorStatus {
    /// The lowest error status.
    pub const MIN: u16 = 400;
    /// The highest error status.
    pub const MAX: u16 = 599;
}

impl TryFrom<i32> for ErrorStatus {
    type Error = ErrorStatusError;

    fn try_from(raw: i32) -> Result<Self, Self::Error> {
        let status = u16::try_from(raw).map_err(|_| ErrorStatusError::OutOfRange)?;
        if !(Self::MIN..=Self::MAX).contains(&status) {
            return Err(ErrorStatusError::OutOfRange);
        }
        Ok(Self(status))
    }
}

/// The display preferences the browser reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DisplaySettings {
    /// The browser asks for reduced motion.
    pub reduced_motion: bool,
    /// The browser asks for more contrast.
    pub more_contrast: bool,
    /// The browser forces its own colors.
    pub forced_colors: bool,
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
