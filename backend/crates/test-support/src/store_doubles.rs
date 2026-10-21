//! Store doubles that answer from a real store until a test scripts them otherwise: an Account
//! listing that also lists a membership the store wouldn't, an Account or user read that fails,
//! a role read that answers a stale role, and a user read that finds no row. A script is shared,
//! so a test seeds through the real answers first and scripts the double afterwards.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::{Account, AccountId, AccountMembership, ListingScope},
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationId},
    role::Role,
    user::{User, UserId},
};
use domain::ports::{AccountStore, UserStore};

/// An [`AccountStore`] read a script can make fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountRead {
    /// `find`, one live Account by id.
    Find,
    /// `list_for_user`, a user's memberships.
    ListForUser,
}

/// Makes the membership a scripted listing adds; a membership is not `Clone`.
type MakeMembership = Arc<dyn Fn() -> AccountMembership + Send + Sync>;

/// What a [`ScriptedAccounts`] does beyond its inner store.
#[derive(Default)]
struct AccountsPlan {
    also_listed: Option<MakeMembership>,
    find_fails: bool,
    list_for_user_fails: bool,
    answered_roles: HashMap<(UserId, AccountId), Role>,
}

impl AccountsPlan {
    /// Whether `read` is scripted to fail.
    fn fails(&mut self, read: AccountRead) -> &mut bool {
        match read {
            AccountRead::Find => &mut self.find_fails,
            AccountRead::ListForUser => &mut self.list_for_user_fails,
        }
    }
}

/// A shared script for [`ScriptedAccounts`]; every clone scripts the same double. Empty, the
/// double answers as its inner store does.
#[derive(Clone, Default)]
pub struct AccountsScript(Arc<Mutex<AccountsPlan>>);

impl AccountsScript {
    /// From now on `list_for_user` also lists the membership `make` makes, between the store's
    /// first and second rows, whoever asks.
    pub fn also_list(&self, make: impl Fn() -> AccountMembership + Send + Sync + 'static) {
        self.plan().also_listed = Some(Arc::new(make));
    }

    /// From now on `read` fails, until [`AccountsScript::read_through`].
    pub fn fail(&self, read: AccountRead) {
        *self.plan().fails(read) = true;
    }

    /// From now on `read` answers from the inner store again.
    pub fn read_through(&self, read: AccountRead) {
        *self.plan().fails(read) = false;
    }

    /// From now on `role_of` answers `role` for `user` in `account`, as a read taken before a
    /// concurrent change would; every other pair reads through.
    pub fn answer_role_of(&self, user: UserId, account: AccountId, role: Role) {
        self.plan().answered_roles.insert((user, account), role);
    }

    fn plan(&self) -> MutexGuard<'_, AccountsPlan> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The role scripted for `user` in `account`, else `None` to read through.
    fn answered_role(&self, user: &UserId, account: &AccountId) -> Option<Role> {
        let pair = (user.clone(), account.clone());
        self.plan().answered_roles.get(&pair).cloned()
    }

    fn check(&self, read: AccountRead) -> anyhow::Result<()> {
        if *self.plan().fails(read) {
            let failure = anyhow::anyhow!("the account store failed ({read:?}, scripted)");
            return Err(failure);
        }
        Ok(())
    }

    fn also_listed(&self) -> Option<AccountMembership> {
        let make = self.plan().also_listed.clone()?;
        Some(make())
    }
}

/// An [`AccountStore`] that answers from `inner` as its [`AccountsScript`] says.
pub struct ScriptedAccounts {
    inner: Arc<dyn AccountStore>,
    script: AccountsScript,
}

impl ScriptedAccounts {
    /// Wraps `inner`, answering as `script` says.
    pub fn new(inner: Arc<dyn AccountStore>, script: AccountsScript) -> Self {
        Self { inner, script }
    }
}

#[async_trait]
impl AccountStore for ScriptedAccounts {
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        self.script.check(AccountRead::Find)?;
        self.inner.find(id).await
    }

    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>> {
        if let Some(role) = self.script.answered_role(user, account) {
            return Ok(Some(role));
        }
        self.inner.role_of(user, account).await
    }

    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        self.inner
            .find_pending_invitation(account, invited_user)
            .await
    }

    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>> {
        self.inner.find_invitation(id).await
    }

    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        self.inner.find_did_by_handle(handle).await
    }

    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        self.inner.count_handle_changes_since(account, since).await
    }

    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        self.inner
            .handle_reserved_for_other(handle, excluding, since)
            .await
    }

    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>> {
        self.script.check(AccountRead::ListForUser)?;
        let mut memberships = self.inner.list_for_user(user, scope).await?;
        if let Some(membership) = self.script.also_listed() {
            let between_first_and_second = memberships.len().min(1);
            memberships.insert(between_first_and_second, membership);
        }
        Ok(memberships)
    }

    async fn list_deactivated_for_owner(&self, owner: &UserId) -> anyhow::Result<Vec<Account>> {
        self.inner.list_deactivated_for_owner(owner).await
    }
}

/// A [`UserStore`] read a script can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRead {
    /// `find`, a user by id.
    Find,
    /// `find_by_did`, a user by DID.
    FindByDid,
}

/// How a scripted user read answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAnswer {
    /// No row, whoever is asked about.
    Gone,
    /// The store fails.
    Fails,
}

/// What a [`ScriptedUsers`] does beyond its inner store.
#[derive(Debug, Default)]
struct UsersPlan {
    find: Option<UserAnswer>,
    find_by_did: Option<UserAnswer>,
}

impl UsersPlan {
    /// How `read` is scripted to answer; `None` reads through.
    fn answer(&mut self, read: UserRead) -> &mut Option<UserAnswer> {
        match read {
            UserRead::Find => &mut self.find,
            UserRead::FindByDid => &mut self.find_by_did,
        }
    }
}

/// A shared script for [`ScriptedUsers`]; every clone scripts the same double. Empty, the double
/// answers as its inner store does.
#[derive(Debug, Clone, Default)]
pub struct UsersScript(Arc<Mutex<UsersPlan>>);

impl UsersScript {
    /// From now on `read` answers as `answer` says, until [`UsersScript::read_through`].
    pub fn answer(&self, read: UserRead, answer: UserAnswer) {
        *self.plan().answer(read) = Some(answer);
    }

    /// From now on `read` answers from the inner store again.
    pub fn read_through(&self, read: UserRead) {
        *self.plan().answer(read) = None;
    }

    fn plan(&self) -> MutexGuard<'_, UsersPlan> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn scripted(&self, read: UserRead) -> Option<UserAnswer> {
        *self.plan().answer(read)
    }
}

/// A [`UserStore`] that answers from `inner` as its [`UsersScript`] says.
pub struct ScriptedUsers {
    inner: Arc<dyn UserStore>,
    script: UsersScript,
}

impl ScriptedUsers {
    /// Wraps `inner`, answering as `script` says.
    pub fn new(inner: Arc<dyn UserStore>, script: UsersScript) -> Self {
        Self { inner, script }
    }

    /// The scripted answer for `read`, else `None` to read through.
    fn scripted(&self, read: UserRead) -> Option<anyhow::Result<Option<User>>> {
        let answer = self.script.scripted(read)?;
        let scripted = match answer {
            UserAnswer::Gone => Ok(None),
            UserAnswer::Fails => Err(anyhow::anyhow!(
                "the user store failed ({read:?}, scripted)"
            )),
        };
        Some(scripted)
    }
}

#[async_trait]
impl UserStore for ScriptedUsers {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        if let Some(scripted) = self.scripted(UserRead::Find) {
            return scripted;
        }
        self.inner.find(id).await
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        if let Some(scripted) = self.scripted(UserRead::FindByDid) {
            return scripted;
        }
        self.inner.find_by_did(did).await
    }
}
