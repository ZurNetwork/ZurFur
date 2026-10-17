use kernel::datetime::DateTimeUtc;
use kernel::elements::did::Did;

use super::UserId;

/// A recognized visitor: their [`Did`] as a [`UserId`], stamped with when
/// Zurfur first saw it. Holds no profile data — handle, display name and avatar
/// are user-owned and fetched from the PDS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    /// When Zurfur first recognized this DID — an explicit domain fact, since
    /// the id carries no timestamp.
    pub created_at: DateTimeUtc,
}

impl User {
    /// The act of first recognition: key the User on `did` and stamp the
    /// moment. A pure value — persisting it, and the one-DID-one-User rule, are
    /// [`crate::ports::UserWrites::provision`]'s job.
    ///
    /// ```
    /// use chrono::Utc;
    /// use identity::elements::user::User;
    /// use kernel::elements::did::Did;
    ///
    /// let user = User::recognize(Did::from("did:plc:example".to_string()), Utc::now());
    /// assert_eq!(user.id.to_string(), "did:plc:example");
    /// ```
    pub fn recognize(did: Did, now: DateTimeUtc) -> Self {
        Self {
            id: UserId::from(did),
            created_at: now,
        }
    }
}
