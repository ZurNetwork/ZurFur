use crate::elements::did::Did;
use crate::elements::profile::DisplayHandle;

/// A visitor's public profile, read from their PDS. `display_name` and
/// `avatar_url` are optional — a PDS may carry neither, and a page must still
/// render from the handle alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub did: Did,
    /// A handle that checked out both ways (it resolves to `did`, and `did`'s
    /// document names it); when it did not, the source stores the DID here or
    /// [`DisplayHandle::INVALID`], never the unconfirmed handle.
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

    /// The names this profile offers for its actor, best first: the display
    /// name, then the handle unless it is the `handle.invalid` marker, then the
    /// DID. A caller takes the first that its own name rule accepts.
    pub fn name_candidates(&self) -> impl Iterator<Item = &str> {
        let display_name = self.display_name.as_deref();
        let handle = Some(&self.handle)
            .filter(|handle| !handle.is_invalid())
            .map(AsRef::<str>::as_ref);
        let did = AsRef::<str>::as_ref(&self.did);
        display_name.into_iter().chain(handle).chain([did])
    }
}

#[cfg(test)]
mod tests;
