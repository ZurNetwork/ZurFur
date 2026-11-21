/// What a custody key is for. Each purpose has its own key.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownPurpose,
    parse_err_fn = unknown_purpose
)]
pub enum Purpose {
    /// Sealing the did:plc custody keys.
    DidCustody,
    /// Sealing sign-in's OAuth secrets.
    SigninSecrets,
    /// Tagging the Den's records.
    DenMac,
}

/// The purposes whose key seals and opens secrets.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownSealPurpose,
    parse_err_fn = unknown_seal_purpose
)]
pub enum SealPurpose {
    /// Sealing the did:plc custody keys.
    DidCustody,
    /// Sealing sign-in's OAuth secrets.
    SigninSecrets,
}

/// What a MAC under the Den's key is computed over.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(
    serialize_all = "snake_case",
    parse_err_ty = UnknownMacUse,
    parse_err_fn = unknown_mac_use
)]
pub enum MacUse {
    /// A Den node.
    DenNode,
    /// A Den changelog entry.
    DenChangelogEntry,
}

impl From<SealPurpose> for Purpose {
    fn from(seal_purpose: SealPurpose) -> Self {
        match seal_purpose {
            SealPurpose::DidCustody => Self::DidCustody,
            SealPurpose::SigninSecrets => Self::SigninSecrets,
        }
    }
}

/// Why a token failed to resolve to a [`Purpose`]. Carries no part of the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: did_custody, signin_secrets, den_mac")]
pub struct UnknownPurpose;

/// Why a token failed to resolve to a [`SealPurpose`]. Carries no part of the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: did_custody, signin_secrets")]
pub struct UnknownSealPurpose;

/// Why a token failed to resolve to a [`MacUse`]. Carries no part of the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("token is not one of: den_node, den_changelog_entry")]
pub struct UnknownMacUse;

/// The typed error for a token outside [`Purpose`]; strum hands it the original input.
fn unknown_purpose(_token: &str) -> UnknownPurpose {
    UnknownPurpose
}

/// The typed error for a token outside [`SealPurpose`]; strum hands it the original input.
fn unknown_seal_purpose(_token: &str) -> UnknownSealPurpose {
    UnknownSealPurpose
}

/// The typed error for a token outside [`MacUse`]; strum hands it the original input.
fn unknown_mac_use(_token: &str) -> UnknownMacUse {
    UnknownMacUse
}

#[cfg(test)]
mod tests;
