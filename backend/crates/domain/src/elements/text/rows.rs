/// Text as the store holds it, loaded without re-checking any rule. It passed
/// the rule in force when it was written, which may be older than today's, so a
/// reader converts it through its own rule before serving it.
///
/// ```
/// use domain::elements::text::StoredText;
///
/// let stored = StoredText::from("  a name written long ago ".to_owned());
/// assert_eq!(stored.as_str(), "  a name written long ago "); // kept verbatim
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::AsRef)]
#[as_ref(str)]
pub struct StoredText(String);

impl StoredText {
    /// The stored text, exactly as loaded.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
