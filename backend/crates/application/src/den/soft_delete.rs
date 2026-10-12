/// The soft-deleted state an open node is marked with.
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
pub enum SoftDelete {
    /// An archived commission.
    Archived,
    /// A deactivated Account.
    Deactivated,
}

#[cfg(test)]
mod tests;
