use adapter_mem::fixture::alice;
use domain::{
    elements::{
        commission::{ChangelogEntry, ChangelogEntryKind, LifecycleStep},
        did::Did,
        user::UserId,
    },
    ports::UnitOfWork,
};
use test_support::pg_app::pg_runtime;

use super::*;
use crate::{
    den::{act::Needs, test_acts::entries_of},
    test_world,
};

// The system's act needs the system, read from its capability's marker, and is a change on its
// commission's place.
#[test]
fn the_late_mark_needs_the_system_and_changes_its_commission() {
    let commission = CommissionId::mint();
    let act = Act { commission };

    let spec = act.spec();

    let place = Place::Commission(CommissionPlace::Commission(commission));
    assert_eq!(spec.needs, Needs::System);
    assert_eq!(spec.needs, needs::<Act>());
    assert_eq!(spec.anchor, Anchor::Thing(place));
    assert_eq!(spec.shape, ActShape::Change);
}

// A write's outcome as the sweep counts it: a landing is a mark, nothing owed and not found are
// skips, and anything else is a failure the sweep logs.
#[test]
fn nothing_owed_and_not_found_are_skips_and_the_rest_fail() {
    let landed: Result<(), DenWriteError> = Ok(());
    let owed_nothing = Err(DenWriteError::Precondition(CommissionRefusal::NothingOwed));
    let gone = Err(DenWriteError::NotFound);
    let failed = Err(DenWriteError::Store(anyhow::anyhow!("the store failed")));
    let disputed = Err(DenWriteError::Precondition(
        CommissionRefusal::CommissionDisputed,
    ));

    assert_eq!(marked(landed).ok(), Some(Marked::Late));
    assert_eq!(marked::<()>(owed_nothing).ok(), Some(Marked::Skipped));
    assert_eq!(marked::<()>(gone).ok(), Some(Marked::Skipped));
    assert!(marked::<()>(failed).is_err());
    assert!(marked::<()>(disputed).is_err());
}

// The sweep's scan leaves out every step a deadline doesn't lapse on: the domain's list.
#[test]
fn the_scan_leaves_out_the_domain_s_non_lapsing_steps() {
    let not_lapsing = LifecycleStep::not_lapsing();

    assert!(not_lapsing.contains(&LifecycleStep::Disputed));
    assert!(not_lapsing.iter().all(|step| !step.lapses()));
}

/// The missed deadline the system's tests give their commission.
const MISSED: &str = "2020-01-01T00:00:00Z";

/// The instant the sweeps here run at, after [`MISSED`].
fn after_missed() -> DateTimeUtc {
    "2020-06-01T00:00:00Z".parse().expect("a timestamp")
}

/// A commission of `owner`'s whose deadline, [`MISSED`], has lapsed, made through the use case.
async fn lapsed(ports: &Ports, owner: &UserId) -> CommissionId {
    let deadline: DateTimeUtc = MISSED.parse().expect("a timestamp");
    let made_at = "2019-12-01T00:00:00Z".parse().expect("a timestamp");
    let command = crate::commission::create::Command {
        actor_id: owner.clone(),
        title: "A lapsed piece".parse().expect("a title"),
        maturity: None,
        deadline: Some(deadline),
    };
    let created = Commissions::new(ports)
        .create(command, made_at)
        .await
        .expect("the commission is made");
    created.id
}

/// The user `did`, recognized.
async fn provision(ports: &Ports, did: &str) -> UserId {
    let did = Did::from(did.to_owned());
    #[allow(clippy::disallowed_methods, reason = "test seeding")]
    let user = crate::transaction(&*ports.database, async move |uow: &mut dyn UnitOfWork| {
        let user = uow.users().provision(&did).await?;
        Ok(user.id)
    })
    .await;
    user.expect("the user is recognized")
}

/// Two sweeps over `ports`: the first appends exactly one `late` entry, by no actor, naming the
/// locked deadline and the flag it supersedes; the second appends none.
async fn one_entry_by_no_actor(ports: &Ports, owner: &UserId) {
    let commission = lapsed(ports, owner).await;

    let first = Commissions::new(ports)
        .sweep_deadlines(after_missed())
        .await
        .expect("the sweep runs");
    let second = Commissions::new(ports)
        .sweep_deadlines(after_missed())
        .await
        .expect("the sweep runs");

    let entries = entries_of(ports, commission).await;
    let late: Vec<&ChangelogEntry> = entries
        .iter()
        .filter(|entry| entry.kind == ChangelogEntryKind::Late)
        .collect();
    assert_eq!(first.marked_late, 1);
    assert_eq!(second.marked_late, 0, "a second pass marks nothing");
    assert_eq!(late.len(), 1, "exactly one late entry");
    assert_eq!(
        late[0].actor_id, None,
        "the system's entry carries no actor"
    );
    let payload = serde_json::json!({ "deadline": MISSED, "from": null });
    assert_eq!(late[0].payload, payload);
    assert_eq!(late[0].note, None);
}

#[tokio::test]
async fn the_system_act_appends_one_entry_by_no_actor() {
    let world = test_world::world().await;

    one_entry_by_no_actor(&world.ports, &alice()).await;
}

#[tokio::test]
async fn the_system_act_appends_one_entry_by_no_actor_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let owner = provision(&ports, "did:plc:sweep-system-owner").await;

    one_entry_by_no_actor(&ports, &owner).await;
}

/// `owner`'s deadline set on `commission` to `deadline`, at [`after_missed`].
async fn set_deadline(
    ports: &Ports,
    owner: &UserId,
    commission: CommissionId,
    deadline: DateTimeUtc,
) {
    let command = crate::commission::deadline::set::Command {
        actor_id: owner.clone(),
        commission_id: commission,
        deadline,
    };
    Commissions::new(ports)
        .deadline()
        .set(command, after_missed())
        .await
        .expect("the deadline set lands");
}

/// A commission of `owner`'s with no deadline, made through the use case.
async fn undated(ports: &Ports, owner: &UserId) -> CommissionId {
    let made_at = "2019-12-01T00:00:00Z".parse().expect("a timestamp");
    let command = crate::commission::create::Command {
        actor_id: owner.clone(),
        title: "An undated piece".parse().expect("a title"),
        maturity: None,
        deadline: None,
    };
    let created = Commissions::new(ports)
        .create(command, made_at)
        .await
        .expect("the commission is made");
    created.id
}

/// A missed deadline set, a sweep that marks it, the same deadline set again one nanosecond
/// later, and a second sweep: the second marks nothing, and the stream ends in the one set and
/// the one `late`.
async fn a_finer_set_re_arms_nothing(ports: &Ports, owner: &UserId) {
    let missed: DateTimeUtc = MISSED.parse().expect("a timestamp");
    let one_nano_later = missed + chrono::Duration::nanoseconds(1);
    let commission = undated(ports, owner).await;
    set_deadline(ports, owner, commission, missed).await;

    let first = Commissions::new(ports)
        .sweep_deadlines(after_missed())
        .await
        .expect("the sweep runs");
    set_deadline(ports, owner, commission, one_nano_later).await;
    let second = Commissions::new(ports)
        .sweep_deadlines(after_missed())
        .await
        .expect("the sweep runs");

    let entries = entries_of(ports, commission).await;
    let kinds: Vec<ChangelogEntryKind> = entries.iter().map(|entry| entry.kind).collect();
    let expected = [
        ChangelogEntryKind::Created,
        ChangelogEntryKind::DeadlineSet,
        ChangelogEntryKind::Late,
    ];
    assert_eq!(first.marked_late, 1);
    assert_eq!(second.marked_late, 0, "the finer set re-arms nothing");
    assert_eq!(kinds, expected);
}

// A deadline set finer than the store holds is no new deadline, so a sweep after it marks
// nothing new.
#[tokio::test]
async fn a_sub_microsecond_move_re_arms_no_late_mark() {
    let world = test_world::world().await;

    a_finer_set_re_arms_nothing(&world.ports, &alice()).await;
}

#[tokio::test]
async fn a_sub_microsecond_move_re_arms_no_late_mark_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let owner = provision(&ports, "did:plc:sweep-finer-owner").await;

    a_finer_set_re_arms_nothing(&ports, &owner).await;
}
