use crate::elements::did::Did;

/// A visitor's public profile, read from their PDS. Handle, display name, and
/// avatar are user-owned data on the public boundary — we read and cache them,
/// we never own them. `display_name` and `avatar_url` are optional: a PDS may
/// carry neither, and the page must still render the handle (ZMVP-10's graceful
/// degradation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub did: Did,
    pub handle: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}
