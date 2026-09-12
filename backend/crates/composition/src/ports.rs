use application::{App, Ports, account::AccountPorts, commission::CommissionPorts};

use crate::Runtime;

impl<'a> From<&'a Runtime> for CommissionPorts<'a> {
    fn from(state: &'a Runtime) -> Self {
        CommissionPorts {
            users: &*state.users,
            did_minter: &*state.did_minter,
            accounts: &*state.accounts,
            commissions: &*state.commissions,
            changelog: &*state.changelog,
            database: &*state.database,
            files: &*state.files,
        }
    }
}

impl<'a> From<&'a Runtime> for AccountPorts<'a> {
    fn from(state: &'a Runtime) -> Self {
        AccountPorts {
            accounts: &*state.accounts,
            users: &*state.users,
            did_minter: &*state.did_minter,
            database: &*state.database,
        }
    }
}

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
            did_minter: state.did_minter.clone(),
            files: state.files.clone(),
            workflows: state.workflows.clone(),
            columns: state.columns.clone(),
        }
    }
}

impl From<&Runtime> for App {
    /// The orchestrator over this runtime. Cheap; build it once at boot.
    fn from(state: &Runtime) -> Self {
        App::new(Ports::from(state))
    }
}
