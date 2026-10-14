use crate::den::NodeKind;

/// Every node type the Den serves, by its wire word; the words are provisional working names.
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
pub enum DenType {
    /// A top-level directory such as `/user`; never part of a viewer's Den path.
    #[strum(serialize = "top_level")]
    TopLevel,
    /// A poster's posts, `/post/<did>`.
    #[strum(serialize = "posts")]
    Posts,
    /// A user's own directory, the viewer's root.
    #[strum(serialize = "user")]
    User,
    /// A user's `accounts` folder.
    #[strum(serialize = "user.accounts")]
    UserAccounts,
    /// A user's `characters` folder.
    #[strum(serialize = "user.characters")]
    UserCharacters,
    /// A user's `commissions` folder.
    #[strum(serialize = "user.commissions")]
    UserCommissions,
    /// An Account's directory.
    #[strum(serialize = "account")]
    Account,
    /// An Account's `characters` folder.
    #[strum(serialize = "account.characters")]
    AccountCharacters,
    /// An Account's `commissions` folder.
    #[strum(serialize = "account.commissions")]
    AccountCommissions,
    /// An Account's `workflows` folder.
    #[strum(serialize = "account.workflows")]
    AccountWorkflows,
    /// A Character's directory.
    #[strum(serialize = "character")]
    Character,
    /// A commission's directory.
    #[strum(serialize = "commission")]
    Commission,
    /// A commission's `attachments` folder.
    #[strum(serialize = "commission.attachments")]
    CommissionAttachments,
    /// A commission's `changelog` file.
    #[strum(serialize = "commission.changelog")]
    CommissionChangelog,
    /// A commission's `products` folder.
    #[strum(serialize = "commission.products")]
    CommissionProducts,
    /// A commission's `slots` folder.
    #[strum(serialize = "commission.slots")]
    CommissionSlots,
    /// One slot of a commission.
    #[strum(serialize = "slot")]
    Slot,
    /// A file, such as an attachment.
    #[strum(serialize = "file")]
    File,
    /// A Workflow, an Account's board.
    #[strum(serialize = "workflow")]
    Workflow,
}

impl DenType {
    /// Whether nodes of this type hold entries or content.
    pub fn kind(self) -> NodeKind {
        match self {
            DenType::CommissionChangelog | DenType::File => NodeKind::File,
            DenType::TopLevel
            | DenType::Posts
            | DenType::User
            | DenType::UserAccounts
            | DenType::UserCharacters
            | DenType::UserCommissions
            | DenType::Account
            | DenType::AccountCharacters
            | DenType::AccountCommissions
            | DenType::AccountWorkflows
            | DenType::Character
            | DenType::Commission
            | DenType::CommissionAttachments
            | DenType::CommissionProducts
            | DenType::CommissionSlots
            | DenType::Slot
            | DenType::Workflow => NodeKind::Directory,
        }
    }

    /// Whether this view doesn't show the content of nodes of this type yet; fixed per type,
    /// never read from a node's data.
    pub fn content_not_shown(self) -> bool {
        match self {
            DenType::Posts
            | DenType::AccountCommissions
            | DenType::Workflow
            | DenType::Slot
            | DenType::File
            | DenType::CommissionChangelog
            | DenType::Character => true,
            DenType::TopLevel
            | DenType::User
            | DenType::UserAccounts
            | DenType::UserCharacters
            | DenType::UserCommissions
            | DenType::Account
            | DenType::AccountCharacters
            | DenType::AccountWorkflows
            | DenType::Commission
            | DenType::CommissionAttachments
            | DenType::CommissionProducts
            | DenType::CommissionSlots => false,
        }
    }
}

#[cfg(test)]
mod tests;
