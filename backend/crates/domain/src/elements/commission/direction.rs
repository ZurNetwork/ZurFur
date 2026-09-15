use super::UnknownDirectionStatus;

/// The direction-axis Status a commission may carry — whose turn the work is
/// waiting on. Always set explicitly by a Participant, never by a content
/// event. One nullable column, so a set replaces and `None` means cleared.
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
#[strum(serialize_all = "snake_case", parse_err_ty = UnknownDirectionStatus, parse_err_fn = unknown_token)]
pub enum DirectionStatus {
    /// The work waits on input from the client side.
    WaitingForInput,
    /// The work waits on an approval.
    WaitingForApproval,
    /// Changes were requested on what was delivered.
    ChangesRequested,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_token(_token: &str) -> UnknownDirectionStatus {
    UnknownDirectionStatus
}

#[cfg(test)]
mod tests;
