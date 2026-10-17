/// The app-private key of a bug report (UUIDv7, so it sorts by filing time).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct BugReportId(uuid::Uuid);

/// The short reference a reporter quotes about their report: the last 8
/// characters of its id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct BugReportReference(String);

impl BugReportReference {
    /// How many characters of the id the reference keeps.
    pub const LEN: usize = 8;
}

impl From<&BugReportId> for BugReportReference {
    /// The id's last [`LEN`](Self::LEN) characters.
    fn from(id: &BugReportId) -> Self {
        let id_text = id.to_string();
        let tail_start = id_text.len() - Self::LEN;
        Self(id_text[tail_start..].to_owned())
    }
}

#[cfg(test)]
mod tests;
