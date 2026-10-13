/// Whether a node holds entries, content, or points at another node.
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
pub enum NodeKind {
    /// It holds entries and can be listed.
    Directory,
    /// It holds content.
    File,
    /// It points at another node; no symlink resolves yet.
    Symlink,
}

#[cfg(test)]
mod tests;
