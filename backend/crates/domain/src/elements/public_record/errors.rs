/// Why an [`AtUri`] string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtUriParseError {
    /// The string did not start with the `at://` scheme.
    MissingScheme,
    /// The string did not have exactly the `authority/collection/rkey` three parts.
    Malformed,
}

impl std::fmt::Display for AtUriParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtUriParseError::MissingScheme => write!(f, "AT-URI must start with `at://`"),
            AtUriParseError::Malformed => {
                write!(f, "AT-URI must be at://<did>/<collection>/<rkey>")
            }
        }
    }
}

impl std::error::Error for AtUriParseError {}
