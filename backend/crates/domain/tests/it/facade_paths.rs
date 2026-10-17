//! Guard: the façade's public surface.
//!
//! Every module and every flat port name `domain` re-exports is named here once,
//! so a path that stops resolving fails to compile, and each flat port name is
//! checked against the crate that defines it.

use std::any::type_name;

/// The re-exported element modules and the top-level modules. The imports are
/// the pin: a module that stops resolving fails to compile.
#[allow(unused_imports, reason = "each import pins one façade path")]
mod element_modules {
    use domain::elements::{
        account, account_keys, achievement, actor_identity, blob, character, commission, did,
        handle, invitation, markdown, maturity, plc_operation, profile, public_record, role, text,
        user, user_account, workflow,
    };
    use domain::{datetime, string_builder};
}

/// The re-exported per-area port modules, pinned the same way, with
/// `UnitOfWorkFn`, a closure bound with no type of its own to name.
#[allow(unused_imports, reason = "each import pins one façade path")]
mod port_modules {
    use domain::ports::{
        UnitOfWorkFn, actor_identity, changelog, character, commission, file, workflow,
    };
}

/// The crate a type's name says it is defined in.
fn defining_crate(name: &str) -> &str {
    let path = name.trim_start_matches("&mut ").trim_start_matches("dyn ");
    path.split("::").next().unwrap_or(path)
}

#[test]
fn every_flat_port_name_resolves_to_its_crate() {
    use domain::ports::{
        AccountMismatch, AccountReads, AccountRepo, AccountStore, AccountWrites,
        ActorIdentityStore, ActorIdentityWrites, Authenticator, BrowserBinding, BrowserMismatch,
        ChangelogStore, ChangelogWrites, CharacterStore, CharacterWrites, ColumnStore,
        ColumnWrites, CommissionReads, CommissionRepo, CommissionStore, CommissionWrites, Database,
        DidBelongsToAnotherActor, DidMinter, ElementNotFound, FileStore, HandleTaken,
        IdentityResolver, KeyStore, PlcOperationLog, ProfileCache, ProfileSource, PublicRecords,
        PublicRecordsError, ResolveError, SigninStarted, Unit, UnitOfWork, UnknownSurface,
        UnknownTab, UserStore, UserWrites, WorkflowStore, WorkflowWrites,
    };

    let identity_names = [
        type_name::<dyn AccountReads>(),
        type_name::<dyn AccountRepo>(),
        type_name::<dyn AccountStore>(),
        type_name::<dyn AccountWrites>(),
        type_name::<dyn ActorIdentityStore>(),
        type_name::<dyn ActorIdentityWrites>(),
        type_name::<dyn Authenticator>(),
        type_name::<BrowserBinding>(),
        type_name::<SigninStarted>(),
        type_name::<dyn CharacterStore>(),
        type_name::<dyn CharacterWrites>(),
        type_name::<AccountMismatch>(),
        type_name::<BrowserMismatch>(),
        type_name::<DidBelongsToAnotherActor>(),
        type_name::<HandleTaken>(),
        type_name::<PublicRecordsError>(),
        type_name::<ResolveError>(),
        type_name::<dyn DidMinter>(),
        type_name::<dyn KeyStore>(),
        type_name::<dyn PlcOperationLog>(),
        type_name::<dyn IdentityResolver>(),
        type_name::<dyn ProfileCache>(),
        type_name::<dyn ProfileSource>(),
        type_name::<dyn PublicRecords>(),
        type_name::<dyn UserStore>(),
        type_name::<dyn UserWrites>(),
    ];
    let workflow_names = [
        type_name::<dyn ChangelogStore>(),
        type_name::<dyn ChangelogWrites>(),
        type_name::<dyn CommissionReads>(),
        type_name::<dyn CommissionRepo>(),
        type_name::<dyn CommissionStore>(),
        type_name::<dyn CommissionWrites>(),
        type_name::<ElementNotFound>(),
        type_name::<UnknownSurface>(),
        type_name::<UnknownTab>(),
        type_name::<dyn FileStore>(),
        type_name::<dyn ColumnStore>(),
        type_name::<dyn ColumnWrites>(),
        type_name::<dyn WorkflowStore>(),
        type_name::<dyn WorkflowWrites>(),
    ];
    let facade_names = [
        type_name::<dyn Database>(),
        type_name::<dyn UnitOfWork>(),
        type_name::<Unit<'static>>(),
    ];

    let misplaced: Vec<&str> = identity_names
        .iter()
        .filter(|name| defining_crate(name) != "identity")
        .chain(
            workflow_names
                .iter()
                .filter(|name| defining_crate(name) != "workflow"),
        )
        .chain(
            facade_names
                .iter()
                .filter(|name| defining_crate(name) != "domain"),
        )
        .copied()
        .collect();
    assert!(
        misplaced.is_empty(),
        "façade port names resolve to the wrong crate: {misplaced:?}"
    );
}
