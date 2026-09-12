//! [`Markdown`] — user-authored rich text. **Stub.**

/// Markdown-formatted text, held as its raw source string. A transparent
/// wrapper with no validation or rendering; it only tags a string as Markdown
/// for type clarity at the boundaries that consume it.
#[derive(Debug, Clone)]
pub struct Markdown(pub String);
