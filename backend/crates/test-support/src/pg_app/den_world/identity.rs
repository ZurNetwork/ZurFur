//! Identity's half of the den world's seeding: the Accounts identity's cases need beyond
//! "Supreme Arts" and "Night Shift", each opt-in, so the world every other suite reads stays
//! as it is.

use chrono::Utc;
use domain::elements::{account::AccountId, did::Did, user::UserId};
use serde_json::json;

use super::{DenWorld, STRANGER_DID, found_account, post_json};

/// An Account DID no seed writes.
const ABSENT_ACCOUNT_DID: &str = "did:plc:absentaccountfixtureaaaa";

/// An Account DID no seed writes.
pub fn absent_account() -> AccountId {
    AccountId::from(Did::from(ABSENT_ACCOUNT_DID.to_owned()))
}

/// The stranger: signed in, and part of nothing.
pub fn stranger() -> UserId {
    UserId::from(Did::from(STRANGER_DID.to_owned()))
}

impl DenWorld {
    /// Seats bob as a Member of alice's `account` through the invitation routes: alice
    /// invites him, and he accepts.
    pub async fn add_bob_as_member(&self, account: &AccountId) {
        let bob = adapter_mem::fixture::bob().to_string();
        let invitation = json!({ "user": bob, "role": "member" });
        let invite_path = format!("/accounts/{account}/invitations");
        let invited = post_json(&self.alice, &self.served, &invite_path, &invitation).await;
        assert_eq!(invited.status(), 201, "alice invites bob to {account}");

        let acceptance = json!({ "listed_on_profile": false });
        let accept_path = format!("/accounts/{account}/invitations/accept");
        let accepted = post_json(&self.bob, &self.served, &accept_path, &acceptance).await;
        assert_eq!(accepted.status(), 200, "bob accepts {account}");
    }

    /// Founds alice's "Closed Studio" with bob as a Member, then deactivates it with a
    /// test-only call of the adapter's soft-delete statement: today's delete always
    /// hard-deletes, since no Account bears a fact yet.
    pub async fn seed_closed_studio(&self) -> AccountId {
        let closed_studio =
            found_account(&self.served, &self.alice, "Closed Studio", "closed-studio").await;
        self.add_bob_as_member(&closed_studio).await;

        let now = Utc::now();
        let id = closed_studio.to_string();
        let deactivated = adapter_pg::queries::account::soft_delete(
            &self.served.runtime.pool,
            Some(now),
            now,
            &id,
        )
        .await
        .expect("deactivate Closed Studio");
        assert_eq!(deactivated, 1, "one Account row is deactivated");
        closed_studio
    }

    /// Founds an Account of alice's under `<label>.zurfur.app`, then writes `stored_name` on
    /// its row with a test-only update, as a name stored before the Den's name rule ran at
    /// write.
    pub async fn seed_account_with_stored_name(&self, label: &str, stored_name: &str) -> AccountId {
        let account = found_account(&self.served, &self.alice, "Placeholder", label).await;
        let updated = sqlx::query("UPDATE accounts SET name = $1 WHERE id = $2")
            .bind(stored_name)
            .bind(account.to_string())
            .execute(&self.served.runtime.pool)
            .await
            .expect("update the Account's name");
        assert_eq!(updated.rows_affected(), 1, "one Account row is updated");
        account
    }
}
