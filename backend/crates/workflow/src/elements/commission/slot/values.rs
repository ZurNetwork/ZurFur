use std::fmt;

use kernel::elements::name::NodeName;

/// A Slot's title: its node's display name, so it parses by the same rule and refuses with the
/// same [`NodeNameError`](kernel::elements::name::NodeNameError).
///
/// ```
/// use workflow::elements::commission::SlotTitle;
///
/// let title = "  The knight  ".parse::<SlotTitle>().unwrap();
/// assert_eq!(title.as_ref(), "The knight"); // stripped
///
/// assert!("   ".parse::<SlotTitle>().is_err()); // empty once stripped
/// ```
#[derive(
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
pub struct SlotTitle(NodeName);

impl fmt::Debug for SlotTitle {
    /// Its type alone; never the title.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SlotTitle(..)")
    }
}

#[cfg(test)]
mod tests;
