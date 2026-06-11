use std::collections::HashSet;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId, blob::BlobId, character::CharacterId, did::Did, golem::GolemId,
        markdown::Markdown, user::UserId,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommissionId(uuid::Uuid);

/// A commission role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommissionRole {
    /// A creator in the commission
    Creator,
    /// A client in the commission
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticipantRef {
    User(UserId),
    Golem(GolemId),
}

#[derive(Debug, Clone)]
pub struct Participant {
    /// The ParticipantRef of the participant. As per design, not an account.
    pub subject: ParticipantRef,
    /// What roles of this participant is
    pub roles: HashSet<CommissionRole>,
    pub assigned: bool,
    /// Who this user was invited by. As per rules as written, Golems may add users!
    pub invited_by: Option<ParticipantRef>,
    /// The title given to this participant
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobRef {
    pub owner: Did,
    pub id: BlobId,
}

#[derive(Debug, Clone)]
pub struct SlotReference {
    pub blob: BlobRef,
    pub notes: Option<Markdown>,
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub slot_number: u8,
    pub character_id: Option<CharacterId>,
    /// Multiple art pieces may be used as references
    pub references: Vec<SlotReference>,
}

#[derive(Debug)]
pub struct Commission {
    /// The ID of the commission
    pub id: CommissionId,
    pub title: String,
    pub description: Markdown,
    /// The current lifecycle state of the commission
    pub lifecycle: Lifecycle,
    pub direction: Option<DirectionStatus>,
    pub deadline_status: Option<DeadlineStatus>,
    /// The account that currently controls the commission. Allows for transferring commissions
    /// between accounts.
    pub current_managing_account_id: Option<AccountId>,
    pub owner: ParticipantRef,
    pub visibility: Visibility,
    /// Every participant and their role in the commission
    pub participants: Vec<Participant>,
    pub deadline: Option<DateTimeUtc>,
    pub slots: Vec<Slot>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
}

/// The lifecycle of the commission.
/// Mutually exclusive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Draft,
    Batched,
    Active,
    Completed,
    Cancelled,
    Disputed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeadlineStatus {
    Delayed,
    Late,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectionStatus {
    WaitingForInput,
    ChangesRequested,
    WaitingForApproval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Private,
    Listed,
    Public,
}
