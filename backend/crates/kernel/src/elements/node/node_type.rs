/// What a node is: a commission, one of its six fixed entries, or a Slot. A closed
/// vocabulary of the words the store holds.
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
#[strum(parse_err_ty = UnknownNodeType, parse_err_fn = unknown_word)]
pub enum NodeType {
    /// A commission, at the top of its tree.
    #[strum(serialize = "commission")]
    Commission,
    /// A commission's attachments entry.
    #[strum(serialize = "commission.attachments")]
    CommissionAttachments,
    /// A commission's brief entry.
    #[strum(serialize = "commission.brief")]
    CommissionBrief,
    /// A commission's changelog entry.
    #[strum(serialize = "commission.changelog")]
    CommissionChangelog,
    /// A commission's participants entry.
    #[strum(serialize = "commission.participants")]
    CommissionParticipants,
    /// A commission's products entry.
    #[strum(serialize = "commission.products")]
    CommissionProducts,
    /// A commission's Slots entry, the folder its Slots sit in.
    #[strum(serialize = "commission.slots")]
    CommissionSlots,
    /// A Slot, under its commission's Slots entry.
    #[strum(serialize = "slot")]
    Slot,
}

/// Why a word failed to resolve to a [`NodeType`]. Carries no part of the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("word is not one of the node types")]
pub struct UnknownNodeType;

/// The typed error for a word outside the vocabulary; strum hands it the original input.
fn unknown_word(_word: &str) -> UnknownNodeType {
    UnknownNodeType
}

#[cfg(test)]
mod tests;
