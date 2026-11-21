use kernel::elements::{name::NodeName, text::StoredText};

/// A commission's Title: a node's display name, so it parses by the same rule and refuses
/// with the same [`NodeNameError`](kernel::elements::name::NodeNameError).
///
/// ```
/// use workflow::elements::commission::CommissionTitle;
///
/// let title = "  A ref sheet  ".parse::<CommissionTitle>().unwrap();
/// assert_eq!(title.as_ref(), "A ref sheet"); // stripped
///
/// assert!("   ".parse::<CommissionTitle>().is_err()); // empty once stripped
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    derive_more::FromStr,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::From,
    derive_more::Into,
)]
#[as_ref(str)]
pub struct CommissionTitle(NodeName);

impl From<CommissionTitle> for StoredText {
    /// The checked title, as the store keeps it.
    fn from(title: CommissionTitle) -> Self {
        StoredText::from(title.to_string())
    }
}

#[cfg(test)]
mod tests;
