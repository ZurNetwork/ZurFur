/// A node's own level, lowest first: absent, a card, or open.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum NodeLevel {
    /// Absent for anyone without a relationship to it.
    Private,
    /// A card for anyone without a relationship to it.
    Listed,
    /// Open to anyone who can reach it.
    Public,
}

#[cfg(test)]
mod tests;
