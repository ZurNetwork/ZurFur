//! The store's refusal of a write into a deactivated Account, read back as
//! [`AccountDeactivated`].

use domain::ports::AccountDeactivated;

/// The constraint name the refusing trigger raises under.
const ACCOUNT_DEACTIVATED: &str = "account_deactivated";

/// `result`, with the store's refusal of a write into a deactivated Account turned into
/// [`AccountDeactivated`]; every other error passes through unchanged.
pub(crate) fn deactivation_aware<T>(result: Result<T, sqlx::Error>) -> anyhow::Result<T> {
    result.map_err(|err| {
        let refused = matches!(
            &err,
            sqlx::Error::Database(db_err) if db_err.constraint() == Some(ACCOUNT_DEACTIVATED)
        );
        if refused {
            anyhow::Error::new(AccountDeactivated)
        } else {
            anyhow::Error::new(err)
        }
    })
}
