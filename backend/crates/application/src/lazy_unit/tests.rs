use std::sync::Arc;

use domain::elements::did::Did;
use domain::ports::Unit;
use test_support::recording::{CallLog, RecordingDatabase};

use super::*;
use crate::{Ports, use_case};

/// A root namespace over a bare [`Ports`] bag, to host throwaway use cases.
#[derive(crate::ports::WithPorts)]
struct Probe<'a> {
    #[ports(is_root = true)]
    ports: &'a Ports,
}

impl Probe<'_> {
    /// Fails on a pool read before it ever opens its unit.
    #[use_case]
    async fn fail_before_open(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] _uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("the pool read refused"))
    }

    /// Never opens its unit and succeeds.
    #[use_case]
    async fn succeed_without_open(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] _uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Opens its unit twice and succeeds.
    #[use_case]
    async fn open_twice(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        uow.open().await?;
        uow.open().await?;
        Ok(())
    }

    /// Opens its unit, then fails.
    #[use_case]
    async fn fail_after_open(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        uow.open().await?;
        Err(anyhow::anyhow!("the write refused"))
    }

    /// Holds an eager unit and calls a lazy use case that opens its own.
    #[use_case]
    async fn eager_around_lazy(&self, #[unit] _uow: Unit<'_>) -> anyhow::Result<()> {
        self.open_twice().await
    }

    /// Opens its unit, then calls a lazy use case that opens its own.
    #[use_case]
    async fn lazy_around_lazy(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        uow.open().await?;
        self.open_twice().await
    }

    /// Delegates to a lazy use case that opens its unit and never opens its own.
    #[use_case]
    async fn delegate_to_lazy(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] _uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        self.open_twice().await
    }

    /// Calls a lazy use case that opens its unit, then opens its own.
    #[use_case]
    async fn lazy_after_lazy(
        &self,
        #[ports] _ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
    ) -> anyhow::Result<()> {
        self.open_twice().await?;
        uow.open().await?;
        Ok(())
    }
}

/// A [`Ports`] bag over the in-memory fakes whose database logs into the
/// returned [`CallLog`]. Assembled by hand: `composition` links the
/// non-test `application`, a distinct `Ports` from this crate's own.
fn recording_ports() -> (Ports, CallLog) {
    let did = Did::from("did:plc:lazy-unit".to_string());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let log = CallLog::default();
    let database = RecordingDatabase::new(runtime.database, log.clone());
    let ports = Ports {
        database: Arc::new(database),
        users: runtime.users,
        accounts: runtime.accounts,
        commissions: runtime.commissions,
        changelog: runtime.changelog,
        profile_source: runtime.profile_source,
        profile_cache: runtime.profile_cache,
        did_minter: runtime.did_minter,
        files: runtime.files,
        workflows: runtime.workflows,
        columns: runtime.columns,
        characters: runtime.characters,
    };
    (ports, log)
}

#[tokio::test]
async fn a_lazy_use_case_that_fails_before_open_begins_no_unit() {
    let (ports, log) = recording_ports();
    let probe = Probe { ports: &ports };

    let outcome = probe.fail_before_open().await;

    assert!(outcome.is_err());
    assert_eq!(log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn a_lazy_use_case_that_never_opens_settles_nothing() {
    let (ports, log) = recording_ports();
    let probe = Probe { ports: &ports };

    probe.succeed_without_open().await.expect("succeeds");

    assert_eq!(log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn opening_twice_begins_once_and_commits_on_ok() {
    let (ports, log) = recording_ports();
    let probe = Probe { ports: &ports };

    probe.open_twice().await.expect("succeeds");

    assert_eq!(log.events(), vec!["begin", "commit"]);
}

#[tokio::test]
async fn an_err_after_open_rolls_the_unit_back() {
    let (ports, log) = recording_ports();
    let probe = Probe { ports: &ports };

    let outcome = probe.fail_after_open().await;

    assert!(outcome.is_err());
    assert_eq!(log.events(), vec!["begin", "rollback"]);
}

#[tokio::test]
async fn a_lazy_use_case_that_never_opens_may_delegate_to_one_that_does() {
    let (ports, log) = recording_ports();
    let probe = Probe { ports: &ports };

    probe.delegate_to_lazy().await.expect("pure delegation");

    assert_eq!(log.events(), vec!["begin", "commit"]);
}

#[cfg(debug_assertions)]
#[tokio::test]
#[should_panic(expected = "opened its own unit after a use case it called")]
async fn opening_a_unit_after_an_inner_use_case_settled_one_trips_the_guard() {
    let (ports, _log) = recording_ports();
    let probe = Probe { ports: &ports };

    let _ = probe.lazy_after_lazy().await;
}

#[cfg(debug_assertions)]
#[tokio::test]
#[should_panic(expected = "opened a unit inside another use case's unit")]
async fn an_eager_unit_around_a_unit_opening_use_case_trips_the_guard() {
    let (ports, _log) = recording_ports();
    let probe = Probe { ports: &ports };

    let _ = probe.eager_around_lazy().await;
}

#[cfg(debug_assertions)]
#[tokio::test]
#[should_panic(expected = "opened a unit inside another use case's unit")]
async fn an_opened_lazy_unit_around_a_unit_opening_use_case_trips_the_guard() {
    let (ports, _log) = recording_ports();
    let probe = Probe { ports: &ports };

    let _ = probe.lazy_around_lazy().await;
}
