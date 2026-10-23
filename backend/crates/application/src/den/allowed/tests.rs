use adapter_mem::{
    CommissionSeed,
    fixture::{alice, bob},
};
use chrono::Utc;
use domain::elements::commission::{CommissionRole, DeadlineStatus};

use super::*;
use crate::{
    commission::{CommissionError, Commissions, archive},
    common_error::CommonError,
    den::test_acts::{FieldCall, field_call, fields_of},
    test_world::{self, World},
};

/// The title every probed commission carries.
const TITLE: &str = "Probed";

/// A deadline from test text.
fn at(text: &str) -> DateTimeUtc {
    text.parse().expect("a timestamp")
}

/// A deadline long past.
fn past() -> DateTimeUtc {
    at("2020-01-01T00:00:00Z")
}

/// A deadline far ahead.
fn future() -> DateTimeUtc {
    at("2099-01-01T00:00:00Z")
}

/// Where a probed commission's deadline lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
enum Deadline {
    None,
    Future,
    Past,
}

/// One state of a probed commission, and who probes it.
#[derive(Debug, Clone)]
struct Case {
    step: LifecycleStep,
    deadline: Deadline,
    delayed: bool,
    role: CommissionRole,
}

impl Case {
    /// The user who holds this case's role: alice owns every probed commission, bob takes part.
    fn actor(&self) -> UserId {
        match self.role {
            CommissionRole::Owner => alice(),
            CommissionRole::Participant => bob(),
        }
    }
}

/// Every lifecycle step, each deadline, the Delayed flag stored or not, for the Owner and for
/// a Participant.
fn every_case() -> Vec<Case> {
    let mut cases = Vec::new();
    for step in LifecycleStep::VARIANTS {
        for deadline in Deadline::VARIANTS {
            for delayed in [false, true] {
                for role in CommissionRole::VARIANTS {
                    let case = Case {
                        step: step.clone(),
                        deadline: *deadline,
                        delayed,
                        role: *role,
                    };
                    cases.push(case);
                }
            }
        }
    }
    cases
}

/// A fresh commission in `world` at `case`'s state, owned by alice, bob a Participant: seeded
/// at Draft, its deadline and flag set through the field acts, then moved to its step.
async fn seeded(world: &World, case: &Case, now: DateTimeUtc) -> CommissionId {
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");
    let mut seed = CommissionSeed::private(title, alice());
    seed.deadline = (case.deadline != Deadline::None || case.delayed).then(future);
    let commission = seed.id;
    world
        .backend
        .seed_bound_commission(&seed)
        .await
        .expect("the commission seeds");
    world.backend.seed_participant(commission, bob(), now);
    let ports = &world.ports;
    if case.delayed {
        field_call(ports, &alice(), commission, FieldCall::SetDelayed, now)
            .await
            .expect("the flag is raised");
    }
    match case.deadline {
        Deadline::None if case.delayed => {
            field_call(ports, &alice(), commission, FieldCall::ClearDeadline, now)
                .await
                .expect("the deadline clears, the flag kept");
        }
        Deadline::Past => {
            let lapsed = FieldCall::SetDeadline(past());
            field_call(ports, &alice(), commission, lapsed, now)
                .await
                .expect("the deadline moves into the past");
        }
        Deadline::None | Deadline::Future => {}
    }
    world
        .backend
        .seed_lifecycle_step(commission, case.step.clone());

    let state = fields_of(ports, commission).await;
    let stored_flag = state.delayed_flag() == Some(DeadlineStatus::Delayed);
    let expected_deadline = match case.deadline {
        Deadline::None => None,
        Deadline::Future => Some(future()),
        Deadline::Past => Some(past()),
    };
    assert_eq!(state.lifecycle_step(), &case.step, "{case:?}");
    assert_eq!(state.deadline(), expected_deadline, "{case:?}");
    assert_eq!(stored_flag, case.delayed, "{case:?}");
    commission
}

/// The acts list for `case`'s actor on `commission`, as the read computes it.
async fn listed_for(
    world: &World,
    case: &Case,
    commission: CommissionId,
    now: DateTimeUtc,
) -> AllowedActs {
    let actor = case.actor();
    let ports = &world.ports;
    let standing = ports
        .commissions
        .standing(&commission, &actor)
        .await
        .expect("the standing reads")
        .expect("the commission stands");
    assert_eq!(standing.role, Some(case.role), "{case:?}");
    let state = fields_of(ports, commission).await;
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");
    allowed_acts(ports, &actor, commission, standing, &state, &title, now).expect("the list")
}

/// One real call a listed act's word stands for.
#[derive(Debug, Clone)]
enum Call {
    Rename,
    Move(LifecycleStep),
    Field(FieldCall),
}

impl Call {
    /// The word of the act this call makes.
    fn word(&self) -> &'static str {
        match self {
            Call::Rename => "rename",
            Call::Move(_) => "move_lifecycle",
            Call::Field(FieldCall::SetDirection(_)) => "set_direction_status",
            Call::Field(FieldCall::ClearDirection) => "clear_direction_status",
            Call::Field(FieldCall::SetDeadline(_)) => "set_deadline",
            Call::Field(FieldCall::ClearDeadline) => "clear_deadline",
            Call::Field(FieldCall::SetDelayed) => "set_delayed",
            Call::Field(FieldCall::ClearDelayed) => "clear_delayed",
        }
    }
}

/// Every call a role holder could make, each value an act takes included.
fn every_call(now: DateTimeUtc) -> Vec<Call> {
    let moves = LifecycleStep::VARIANTS
        .iter()
        .map(|step| Call::Move(step.clone()));
    let direction_sets = DirectionStatus::VARIANTS
        .iter()
        .map(|status| Call::Field(FieldCall::SetDirection(*status)));
    let fields = [
        FieldCall::ClearDirection,
        FieldCall::SetDeadline(now),
        FieldCall::SetDeadline(future()),
        FieldCall::SetDeadline(past()),
        FieldCall::ClearDeadline,
        FieldCall::SetDelayed,
        FieldCall::ClearDelayed,
    ]
    .into_iter()
    .map(Call::Field);
    std::iter::once(Call::Rename)
        .chain(moves)
        .chain(direction_sets)
        .chain(fields)
        .collect()
}

/// What a real call answered: refused, or landed with or without a new changelog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answered {
    Refused,
    Unchanged,
    Changed,
}

/// `call` made for real by `actor` on `commission` at `now`, through its typed use case.
async fn answered(
    world: &World,
    actor: &UserId,
    commission: CommissionId,
    call: &Call,
    now: DateTimeUtc,
) -> Answered {
    let ports = &world.ports;
    let before = ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
        .len();
    let commissions = Commissions::new(ports);
    let result = match call {
        Call::Rename => {
            let command = crate::commission::rename::Command {
                actor_id: actor.clone(),
                commission_id: commission,
                title: TITLE.parse().expect("a valid title"),
            };
            commissions.rename(command, now).await.map(|_| ())
        }
        Call::Move(to) => {
            let command = move_to::Command {
                actor_id: actor.clone(),
                commission_id: commission,
                to: to.clone(),
            };
            commissions
                .lifecycle()
                .move_to(command, now)
                .await
                .map(|_| ())
        }
        Call::Field(field) => field_call(ports, actor, commission, *field, now).await,
    };
    if let Err(CommissionError::CommonError(CommonError::Infrastructure(failure))) = &result {
        panic!("{call:?} failed in the store: {failure:?}");
    }
    let after = ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
        .len();
    match (result, after > before) {
        (Err(_), _) => Answered::Refused,
        (Ok(()), false) => Answered::Unchanged,
        (Ok(()), true) => Answered::Changed,
    }
}

// On every lifecycle step, with no deadline, one ahead or one past, the Delayed flag stored or
// not, for the Owner and for a Participant: an act is listed exactly when some real call of it
// lands, each on a fresh commission at that state, and a step is a target exactly when the real
// move to it lands and records a change. Disputed is simply the Disputed step.
#[tokio::test]
async fn the_acts_list_and_the_gate_agree_on_every_state() {
    let now = Utc::now();
    let calls = every_call(now);
    let mut lists_every_act = false;

    for case in every_case() {
        let world = test_world::world().await;
        let probed = seeded(&world, &case, now).await;
        let allowed = listed_for(&world, &case, probed, now).await;
        let listed: Vec<&str> = allowed.acts.iter().map(AsRef::as_ref).collect();

        let mut landed: Vec<&'static str> = Vec::new();
        let mut targets = Vec::new();
        for call in &calls {
            let fresh = seeded(&world, &case, now).await;
            let answer = answered(&world, &case.actor(), fresh, call, now).await;
            if answer != Answered::Refused && !landed.contains(&call.word()) {
                landed.push(call.word());
            }
            if let (Call::Move(step), Answered::Changed) = (call, answer) {
                targets.push(step.clone());
            }
        }

        let mut listed_sorted = listed.clone();
        listed_sorted.sort_unstable();
        landed.sort_unstable();
        assert_eq!(listed_sorted, landed, "{case:?}");
        assert_eq!(allowed.lifecycle_targets, targets, "{case:?}");
        assert!(
            !allowed.lifecycle_targets.contains(&case.step),
            "the held step is no target: {case:?}"
        );
        lists_every_act |= listed.len() == 8;
    }
    assert!(lists_every_act, "no state lists every act");
}

// The list keeps one fixed order: the rename, the move, then the direction, deadline and
// Delayed acts, each set before its clear.
#[tokio::test]
async fn the_owner_s_acts_come_in_the_contract_s_order() {
    let world = test_world::world().await;
    let now = Utc::now();
    let case = Case {
        step: LifecycleStep::Active,
        deadline: Deadline::Future,
        delayed: true,
        role: CommissionRole::Owner,
    };
    let probed = seeded(&world, &case, now).await;

    let allowed = listed_for(&world, &case, probed, now).await;

    let words: Vec<&str> = allowed.acts.iter().map(AsRef::as_ref).collect();
    let expected_words = [
        "rename",
        "move_lifecycle",
        "set_direction_status",
        "clear_direction_status",
        "set_deadline",
        "clear_deadline",
        "set_delayed",
        "clear_delayed",
    ];
    assert_eq!(words, expected_words);
    let expected_targets = [
        LifecycleStep::Batched,
        LifecycleStep::Completed,
        LifecycleStep::Cancelled,
    ];
    assert_eq!(allowed.lifecycle_targets, expected_targets);
}

// Someone who holds no role may do nothing, whatever the commission's state.
#[tokio::test]
async fn no_role_lists_nothing() {
    let world = test_world::world().await;
    let now = Utc::now();
    let case = Case {
        step: LifecycleStep::Active,
        deadline: Deadline::Future,
        delayed: false,
        role: CommissionRole::Owner,
    };
    let probed = seeded(&world, &case, now).await;
    let stranger = UserId::from(domain::elements::did::Did::from(
        "did:plc:allowed-stranger".to_owned(),
    ));
    let standing = world
        .ports
        .commissions
        .standing(&probed, &stranger)
        .await
        .expect("the standing reads")
        .expect("the commission stands");
    let state = fields_of(&world.ports, probed).await;
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");

    let allowed = allowed_acts(
        &world.ports,
        &stranger,
        probed,
        standing,
        &state,
        &title,
        now,
    )
    .expect("the list");

    assert_eq!(standing.role, None);
    assert_eq!(allowed, AllowedActs::default());
}

// An archived commission refuses nothing: archiving it leaves the Owner's list as it was.
#[tokio::test]
async fn archiving_changes_nothing_in_the_list() {
    let world = test_world::world().await;
    let now = Utc::now();
    let case = Case {
        step: LifecycleStep::Active,
        deadline: Deadline::Future,
        delayed: false,
        role: CommissionRole::Owner,
    };
    let probed = seeded(&world, &case, now).await;
    let live = listed_for(&world, &case, probed, now).await;
    let command = archive::Command {
        actor_id: alice(),
        commission_id: probed,
    };
    Commissions::new(&world.ports)
        .archive(command, now)
        .await
        .expect("alice archives it");

    let archived = listed_for(&world, &case, probed, now).await;

    assert_eq!(archived, live);
}
