/// A handle as the network reports it, carried for display only. Never
/// validated or verified here: the PDS is the authority, and `handle.invalid`
/// is a legitimate value. Not a `Handle`, which is a handle Zurfur has claimed.
///
/// ```
/// use domain::elements::profile::DisplayHandle;
///
/// let handle = DisplayHandle::from("alice.bsky.social");
/// assert_eq!(handle.to_string(), "alice.bsky.social");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayHandle(String);

impl From<String> for DisplayHandle {
    fn from(handle: String) -> Self {
        Self(handle)
    }
}

impl From<&str> for DisplayHandle {
    fn from(handle: &str) -> Self {
        Self(handle.to_string())
    }
}

impl AsRef<str> for DisplayHandle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DisplayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
