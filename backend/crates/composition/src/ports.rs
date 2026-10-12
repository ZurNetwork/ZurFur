use application::{App, Ports};

use crate::Runtime;

impl From<&Runtime> for Ports {
    /// The orchestrator's bag over this runtime's live adapters.
    fn from(state: &Runtime) -> Self {
        Ports {
            database: state.database.clone(),
            users: state.users.clone(),
            accounts: state.accounts.clone(),
            commissions: state.commissions.clone(),
            changelog: state.changelog.clone(),
            profile_source: state.profile_source.clone(),
            profile_cache: state.profile_cache.clone(),
            identity_resolver: state.identity_resolver.clone(),
            did_minter: state.did_minter.clone(),
            files: state.files.clone(),
            workflows: state.workflows.clone(),
            columns: state.columns.clone(),
            characters: state.characters.clone(),
        }
    }
}

impl From<&Runtime> for App {
    /// The orchestrator over this runtime. Cheap; build it once at boot.
    fn from(state: &Runtime) -> Self {
        App::new(Ports::from(state))
    }
}
