//! The in-memory runtime's ports as this crate's own bag, for unit tests: the dev graph links
//! a second copy of this crate through `composition`, so the runtime's own `App` is not this
//! crate's.

use crate::Ports;

impl From<&composition::Runtime> for Ports {
    /// The runtime's ports, each shared.
    fn from(runtime: &composition::Runtime) -> Self {
        Ports {
            database: runtime.database.clone(),
            users: runtime.users.clone(),
            accounts: runtime.accounts.clone(),
            commissions: runtime.commissions.clone(),
            changelog: runtime.changelog.clone(),
            profile_source: runtime.profile_source.clone(),
            profile_cache: runtime.profile_cache.clone(),
            // The runtime's set is the other copy's type; each bag starts its own.
            profile_lookups: crate::user::ProfileLookups::default(),
            identity_resolver: runtime.identity_resolver.clone(),
            did_minter: runtime.did_minter.clone(),
            files: runtime.files.clone(),
            workflows: runtime.workflows.clone(),
            columns: runtime.columns.clone(),
            characters: runtime.characters.clone(),
            bug_reports: runtime.bug_reports.clone(),
        }
    }
}
