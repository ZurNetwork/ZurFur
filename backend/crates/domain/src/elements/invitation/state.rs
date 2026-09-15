use super::UnknownInvitationState;

/// Where an invitation sits in its lifecycle. Pending from issuance until
/// accepted or revoked; both end states are terminal and there is no expiry.
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
#[strum(
    serialize_all = "snake_case",
    ascii_case_insensitive,
    parse_err_ty = UnknownInvitationState,
    parse_err_fn = unknown_state
)]
pub enum InvitationState {
    /// Issued and awaiting the invited User's decision — the only state that
    /// may be revoked or accepted.
    Pending,
    /// The invited User accepted and the membership was minted. Terminal.
    Accepted,
    /// The issuer revoked the offer before it was accepted. Terminal.
    Revoked,
}

/// The typed error for a token outside the vocabulary; strum hands it the original input.
fn unknown_state(token: &str) -> UnknownInvitationState {
    UnknownInvitationState(token.into())
}

#[cfg(test)]
mod tests;
