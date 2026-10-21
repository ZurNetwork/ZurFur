//! Seating a member through the store port, the way production seats one.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId, invitation::Invitation, role::Role, user::UserId,
        user_account::UserAccount,
    },
    ports::UnitOfWork,
};

/// Seat `member` in `account` at `role` on `uow`: issue an invitation from
/// `inviter`, store it, and accept it, always listed. On PostgreSQL the inviter
/// becomes the member's parent, and both must already be Users.
pub async fn seat_member(
    uow: &mut dyn UnitOfWork,
    account: &AccountId,
    member: &UserId,
    role: Role,
    inviter: &UserId,
    now: DateTimeUtc,
) -> anyhow::Result<UserAccount> {
    let issued = Invitation::issue(account.clone(), member.clone(), role, inviter.clone(), now);
    let invitation = uow.accounts().create_invitation(&issued).await?;
    uow.accounts().accept_invitation(invitation, true).await
}

#[cfg(test)]
mod tests;
