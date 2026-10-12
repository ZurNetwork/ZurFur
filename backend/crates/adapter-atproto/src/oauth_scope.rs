//! The OAuth scope sign-in asks for, and the check that a grant is exactly it.

/// OAuth scopes requested at sign-in: `atproto` alone, which authenticates the
/// account and asks for no access to its repo. Write access is never requested
/// here.
pub(crate) const OAUTH_SCOPES: &str = "atproto";

/// Whether `granted`, the token response's `scope`, is exactly
/// [`OAUTH_SCOPES`]. A missing scope, a wider one and a narrower one are all
/// refused.
pub(crate) fn grants_identity_only(granted: Option<&str>) -> bool {
    let Some(granted) = granted else {
        return false;
    };
    granted
        .split_whitespace()
        .eq(OAUTH_SCOPES.split_whitespace())
}

#[cfg(test)]
mod tests;
