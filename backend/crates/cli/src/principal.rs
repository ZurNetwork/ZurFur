//! The one way a command learns who it acts as (ZMVP-203): the identity file
//! → [`UserStore::find_by_did`] → the [`User`]. Every operation command takes
//! a [`Principal`]; none re-implements this.

use std::path::Path;

use composition::Runtime;
use domain::elements::{did::Did, user::User};

use crate::{CliError, identity};

/// The acting User, resolved against the runtime's database.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user: User,
}

impl Principal {
    /// Resolve from the identity file at `path`. Domain problems, exit `1`:
    /// `not_authenticated` (no file, or the DID is unknown to this database),
    /// `identity_mismatch` (recorded against a different database),
    /// `identity_corrupt` (not a valid record). Infrastructure, exit `3`:
    /// `identity_unreadable` (the file cannot be read), `database`.
    pub async fn resolve(runtime: &Runtime, path: &Path) -> Result<Self, CliError> {
        let Some(identity) = identity::load(path)? else {
            return Err(CliError::domain(
                "not_authenticated",
                "no identity recorded; run `zurfur session login` first",
            ));
        };
        let expected = identity::fingerprint(&runtime.config.database_url);
        if identity.database_fingerprint != expected {
            return Err(CliError::domain(
                "identity_mismatch",
                "the recorded identity belongs to a different database; run `zurfur session logout` then `login` against this one",
            ));
        }
        // `identity::load` already parsed it; re-parsing here keeps the
        // trusted/untrusted split visible at the type level and never `new`s
        // file-sourced text.
        let did: Did = identity
            .did
            .parse()
            .map_err(|e| CliError::domain("identity_corrupt", format!("recorded identity: {e}")))?;
        let user = runtime
            .users
            .find_by_did(&did)
            .await
            .map_err(|e| CliError::infra("internal_error", e))?
            .ok_or_else(|| {
                CliError::domain(
                    "not_authenticated",
                    "the recorded identity is unknown to this database; run `zurfur session login` again",
                )
            })?;
        Ok(Principal { user })
    }
}
