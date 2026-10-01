/// Why an [`AtUri`] string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AtUriParseError {
    /// The string did not start with the `at://` scheme.
    #[error("AT-URI must start with `at://`")]
    MissingScheme,
    /// The string did not have exactly the `authority/collection/rkey` three parts.
    #[error("AT-URI must be at://<did>/<collection>/<rkey>")]
    Malformed,
}
