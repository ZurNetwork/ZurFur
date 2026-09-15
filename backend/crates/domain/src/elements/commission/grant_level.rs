//! Commission positioning: the account-facing rails that replaced the deleted
//! managing-account concept. Users own commissions; neither rail confers any
//! in-commission authority.
//!
//! Placement is not here — a commission's placement IS its card on an account's
//! board, so it lives in `domain::elements::workflow`. What remains is
//! [`GrantLevel`], the level of a commission-side key to see.

/// The level a view grant confers — one of the three raw root modes, explicitly
/// chosen at grant time with no default. A grant is issued to a **User**, never
/// an account, and a user's effective view is the max of their own standing and
/// their own key; membership confers no view. Hard-deleted on revoke.
///
/// Not the [`Visibility`](super::Visibility) aliases: a grant speaks the
/// underlying mode directly.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case", parse_err_ty = GrantLevelError, parse_err_fn = unknown_token)]
pub enum GrantLevel {
    /// The narrowest key: the Presentation-mode projection.
    Presentation,
    /// A middle key: whatever is composed under Description-visible surfaces.
    Description,
    /// The widest key: Participant-equivalent view.
    Total,
}

#[derive(Debug, thiserror::Error)]
#[error("Grant level parsing error")]
pub struct GrantLevelError;

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> GrantLevelError {
    GrantLevelError
}

#[cfg(test)]
mod tests;
