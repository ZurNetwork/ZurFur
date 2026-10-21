use super::{BrowserText, BugDescription, BuildId, ErrorStatus, PageAddress, PageKind, WindowSide};

/// Why a text was refused as a [`BugDescription`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BugDescriptionError {
    /// Empty once trimmed.
    #[error("a bug description is empty")]
    Empty,
    /// Longer than [`BugDescription::MAX_CHARS`] characters once trimmed.
    #[error(
        "a bug description is longer than {} characters",
        BugDescription::MAX_CHARS
    )]
    TooLong,
    /// Holds a control character other than newline or tab, or a bidirectional formatting
    /// character.
    #[error("a bug description holds a control or bidirectional formatting character")]
    ControlCharacter,
}

/// Why a text was refused as a [`PageAddress`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageAddressError {
    /// Empty once trimmed.
    #[error("a page is empty")]
    Empty,
    /// Longer than [`PageAddress::MAX_BYTES`] bytes once trimmed.
    #[error("a page is longer than {} bytes", PageAddress::MAX_BYTES)]
    TooLong,
    /// Holds a control, bidirectional formatting, line separator or paragraph separator
    /// character.
    #[error("a page holds a control or bidirectional formatting character")]
    ControlCharacter,
    /// Holds a `\`, which a browser reads as `/`.
    #[error("a page holds no backslash")]
    Backslash,
    /// Doesn't start with `/`.
    #[error("a page starts with `/`")]
    NotAbsolute,
    /// Starts with `//`, which a browser reads as another host.
    #[error("a page does not start with `//`")]
    ProtocolRelative,
    /// Holds a `?` or a `#`: a query or a fragment.
    #[error("a page holds no query and no fragment")]
    QueryOrFragment,
}

/// Why a text was refused as a [`PageKind`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageKindError {
    /// Empty once trimmed.
    #[error("a page kind is empty")]
    Empty,
    /// Longer than [`PageKind::MAX_BYTES`] bytes once trimmed.
    #[error("a page kind is longer than {} bytes", PageKind::MAX_BYTES)]
    TooLong,
    /// Holds a control, bidirectional formatting, line separator or paragraph separator
    /// character.
    #[error("a page kind holds a control or bidirectional formatting character")]
    ControlCharacter,
}

/// Why a text was refused as a [`BuildId`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BuildIdError {
    /// Empty once trimmed.
    #[error("a build id is empty")]
    Empty,
    /// Longer than [`BuildId::MAX_BYTES`] bytes once trimmed.
    #[error("a build id is longer than {} bytes", BuildId::MAX_BYTES)]
    TooLong,
    /// Holds a control, bidirectional formatting, line separator or paragraph separator
    /// character.
    #[error("a build id holds a control or bidirectional formatting character")]
    ControlCharacter,
}

/// Why a text was refused as a [`BrowserText`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BrowserTextError {
    /// Empty once trimmed.
    #[error("a browser is empty")]
    Empty,
    /// Longer than [`BrowserText::MAX_BYTES`] bytes once trimmed.
    #[error("a browser is longer than {} bytes", BrowserText::MAX_BYTES)]
    TooLong,
    /// Holds a control, bidirectional formatting, line separator or paragraph separator
    /// character.
    #[error("a browser holds a control or bidirectional formatting character")]
    ControlCharacter,
}

/// Why a number was refused as a [`WindowSide`]. Carries no part of the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WindowSideError {
    /// Below [`WindowSide::MIN`] or above [`WindowSide::MAX`].
    #[error("a window side is {} to {} pixels", WindowSide::MIN, WindowSide::MAX)]
    OutOfRange,
}

/// Why a number was refused as an [`ErrorStatus`]. Carries no part of the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErrorStatusError {
    /// Below [`ErrorStatus::MIN`] or above [`ErrorStatus::MAX`]: not an HTTP error status.
    #[error("an error status is {} to {}", ErrorStatus::MIN, ErrorStatus::MAX)]
    OutOfRange,
}
