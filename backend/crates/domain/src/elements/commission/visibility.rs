use super::UnknownVisibility;

/// Who may see a commission — the outermost gate, applied before the
/// composition's own [`effective_visibility`]. A fresh commission is
/// [`Private`](Visibility::Private); widening is an explicit later act.
#[derive(
    Debug,
    Clone,
    PartialEq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case", parse_err_ty = UnknownVisibility, parse_err_fn = unknown_token)]
pub enum Visibility {
    /// Nobody outside the participants sees it at all, not even its existence.
    Private,
    /// Outsiders see only a status-only card.
    Listed,
    /// Outsiders see whatever sits under Description-visible surfaces.
    Public,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> UnknownVisibility {
    UnknownVisibility
}

#[cfg(test)]
mod tests;
