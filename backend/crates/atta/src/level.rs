/// How visible a node is, lowest first: absent, a card that can't be opened, or open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Absent: the node doesn't appear at all.
    Private,
    /// A card: the node appears but can't be opened.
    Listed,
    /// Open.
    Public,
}

#[cfg(test)]
mod tests;
