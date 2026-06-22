//! [`Markdown`] — user-authored rich text. **Stub.**
//!
//! A newtype marking a string as Markdown-formatted body text (e.g. a
//! commission's description or a slot reference's notes). Carries no parsing or
//! sanitization yet — that is deferred dressing for when the rendering surface
//! lands.

/// Markdown-formatted text, held as its raw source string.
///
/// Stub: a transparent wrapper with no validation or rendering. It only tags a
/// string as "this is Markdown" for type clarity at the boundaries that consume
/// it (see [`crate::elements::commission`]).
#[derive(Debug, Clone)]
pub struct Markdown(pub String);
