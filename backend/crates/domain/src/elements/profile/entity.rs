use crate::elements::did::Did;
use crate::elements::profile::DisplayHandle;

/// A visitor's public profile, read from their PDS. `display_name` and
/// `avatar_url` are optional — a PDS may carry neither, and a page must still
/// render from the handle alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub did: Did,
    pub handle: DisplayHandle,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl Profile {
    /// A profile with the two facts every PDS carries and nothing optional.
    pub fn new(did: Did, handle: impl Into<DisplayHandle>) -> Self {
        Self {
            did,
            handle: handle.into(),
            display_name: None,
            avatar_url: None,
        }
    }

    /// The same profile with a display name.
    pub fn with_display_name(self, display_name: impl Into<String>) -> Self {
        Self {
            display_name: Some(display_name.into()),
            ..self
        }
    }

    /// The same profile with an avatar URL.
    pub fn with_avatar_url(self, avatar_url: impl Into<String>) -> Self {
        Self {
            avatar_url: Some(avatar_url.into()),
            ..self
        }
    }
}
