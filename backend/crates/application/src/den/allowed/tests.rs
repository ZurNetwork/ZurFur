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

/// One state of a probed commission, and who probes it: the step it holds, or left when
/// disputed.
#[derive(Debug, Clone)]
struct Case {
    step: LifecycleStep,
    disputed: bool,
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

/// Every lifecycle step but Disputed, disputed from it or not, each deadline, the Delayed flag
/// stored or not, for the Owner and for a Participant.
fn every_case() -> Vec<Case> {
    let mut cases = Vec::new();
    let steps = LifecycleStep::VARIANTS
        .iter()
        .filter(|step| **step != LifecycleStep::Disputed);
    for step in steps {
        for disputed in [false, true] {
            for deadline in Deadline::VARIANTS {
                for delayed in [false, true] {
                    for role in CommissionRole::VARIANTS {
                        let case = Case {
                            step: step.clone(),
                            disputed,
                            deadline: *deadline,
                            delayed,
                            role: *role,
                        };
                        cases.push(case);
                    }
                }
            }
        }
    }
    cases
}

/// A fresh commission in `world` at `case`'s state, owned by alice, bob a Participant: seeded
/// at Draft, its deadline and flag set through the field acts, moved to its step, then, when
/// the case is disputed, disputed from it through the raise at `now`.
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
    if case.disputed {
        field_call(ports, &alice(), commission, FieldCall::RaiseDispute, now)
            .await
            .expect("the dispute is raised");
    }

    let state = fields_of(ports, commission).await;
    let stored_flag = state.delayed_flag() == Some(DeadlineStatus::Delayed);
    let expected_deadline = match case.deadline {
        Deadline::None => None,
        Deadline::Future => Some(future()),
        Deadline::Past => Some(past()),
    };
    let held_step = if case.disputed {
        LifecycleStep::Disputed
    } else {
        case.step.clone()
    };
    let left = state.dispute().map(|dispute| dispute.left.clone());
    assert_eq!(state.lifecycle_step(), &held_step, "{case:?}");
    assert_eq!(left, case.disputed.then(|| case.step.clone()), "{case:?}");
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

/// One real call an act the list tries stands for.
#[derive(Debug, Clone)]
enum Call {
    Rename(CommissionTitle),
    Move(LifecycleStep),
    Field(FieldCall),
}

/// One real call, and the word of the act it makes.
#[derive(Debug, Clone)]
struct WordedCall {
    word: &'static str,
    call: Call,
}

/// The real call that makes `act`, through its typed use case.
fn call_of(act: CommissionAct) -> Call {
    match act {
        CommissionAct::Create(_) | CommissionAct::MarkLate(_) => {
            panic!("the list tries no create and no Late mark")
        }
        CommissionAct::Rename(rename) => Call::Rename(rename.title),
        CommissionAct::MoveLifecycle(move_to) => Call::Move(move_to.to),
        CommissionAct::RaiseDispute(_) => Call::Field(FieldCall::RaiseDispute),
        CommissionAct::ClearDispute(_) => Call::Field(FieldCall::ClearDispute),
        CommissionAct::SetDirection(set) => Call::Field(FieldCall::SetDirection(set.to)),
        CommissionAct::ClearDirection(_) => Call::Field(FieldCall::ClearDirection),
        CommissionAct::SetDeadline(set) => {
            let deadline = DateTimeUtc::from(set.deadline);
            Call::Field(FieldCall::SetDeadline(deadline))
        }
        CommissionAct::ClearDeadline(_) => Call::Field(FieldCall::ClearDeadline),
        CommissionAct::SetDelayed(_) => Call::Field(FieldCall::SetDelayed),
        CommissionAct::ClearDelayed(_) => Call::Field(FieldCall::ClearDelayed),
    }
}

/// `act` as a real call, with its word.
fn worded(act: CommissionAct) -> WordedCall {
    let word = act.spec().word;
    let call = call_of(act);
    WordedCall { word, call }
}

/// The calls of the act of `kind` the list should try, written apart from the list: every
/// lifecycle step, every direction status, the rename to the shown title, a deadline at `now` as
/// the set act stores it, the dispute's raise and clear, each clear and Delayed act once, and no
/// create and no Late mark.
fn expected_calls(kind: CommissionActKind, now: DateTimeUtc) -> Vec<Call> {
    match kind {
        CommissionActKind::Create | CommissionActKind::MarkLate => Vec::new(),
        CommissionActKind::Rename => {
            let title = TITLE.parse::<CommissionTitle>().expect("a valid title");
            vec![Call::Rename(title)]
        }
        CommissionActKind::MoveLifecycle => LifecycleStep::VARIANTS
            .iter()
            .map(|step| Call::Move(step.clone()))
            .collect(),
        CommissionActKind::RaiseDispute => vec![Call::Field(FieldCall::RaiseDispute)],
        CommissionActKind::ClearDispute => vec![Call::Field(FieldCall::ClearDispute)],
        CommissionActKind::SetDirection => DirectionStatus::VARIANTS
            .iter()
            .map(|status| Call::Field(FieldCall::SetDirection(*status)))
            .collect(),
        CommissionActKind::ClearDirection => vec![Call::Field(FieldCall::ClearDirection)],
        CommissionActKind::SetDeadline => {
            let stored_now = DateTimeUtc::from(CommissionDeadline::from(now));
            vec![Call::Field(FieldCall::SetDeadline(stored_now))]
        }
        CommissionActKind::ClearDeadline => vec![Call::Field(FieldCall::ClearDeadline)],
        CommissionActKind::SetDelayed => vec![Call::Field(FieldCall::SetDelayed)],
        CommissionActKind::ClearDelayed => vec![Call::Field(FieldCall::ClearDelayed)],
    }
}

/// The words of the acts a user may take on a stored commission: each kind the list should
/// try, sorted.
fn user_acts(now: DateTimeUtc) -> Vec<&'static str> {
    let commission = CommissionId::mint();
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");
    let mut words: Vec<&'static str> = CommissionActKind::VARIANTS
        .iter()
        .filter(|kind| !expected_calls(**kind, now).is_empty())
        .filter_map(|kind| tried(*kind, commission, &title, now).into_iter().next())
        .map(|act| act.spec().word)
        .collect();
    words.sort_unstable();
    words
}

/// Every call the list tries on `commission` at `now`, and a deadline ahead and one past
/// besides: every value the list judges is made for real.
fn every_call(commission: CommissionId, now: DateTimeUtc) -> Vec<WordedCall> {
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");
    let listed = CommissionActKind::VARIANTS
        .iter()
        .flat_map(|kind| tried(*kind, commission, &title, now));
    let other_deadlines = [future(), past()].map(|deadline| {
        let act = deadline::set::Act {
            commission,
            deadline: CommissionDeadline::from(deadline),
        };
        CommissionAct::SetDeadline(act)
    });
    listed.chain(other_deadlines).map(worded).collect()
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
        Call::Rename(title) => {
            let command = crate::commission::rename::Command {
                actor_id: actor.clone(),
                commission_id: commission,
                title: title.clone(),
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

// On every lifecycle step, disputed from it or not, with no deadline, one ahead or one past,
// the Delayed flag stored or not, for the Owner and for a Participant: an act is listed exactly
// when some real call of it lands, each on a fresh commission at that state, and a step is a
// target exactly when the real move to it lands and records a change. Every user act is listed
// on some state; the raise and the clear never on the same one.
#[tokio::test]
async fn the_acts_list_and_the_gate_agree_on_every_state() {
    let now = Utc::now();
    let user_acts = user_acts(now);
    let mut listed_somewhere: Vec<&'static str> = Vec::new();

    for case in every_case() {
        let world = test_world::world().await;
        let probed = seeded(&world, &case, now).await;
        let allowed = listed_for(&world, &case, probed, now).await;
        let listed: Vec<&str> = allowed.acts.iter().map(AsRef::as_ref).collect();

        let mut landed: Vec<&'static str> = Vec::new();
        let mut targets = Vec::new();
        for WordedCall { word, call } in every_call(probed, now) {
            let fresh = seeded(&world, &case, now).await;
            let answer = answered(&world, &case.actor(), fresh, &call, now).await;
            if answer != Answered::Refused && !landed.contains(&word) {
                landed.push(word);
            }
            if let (Call::Move(step), Answered::Changed) = (call, answer) {
                targets.push(step);
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
        let both = listed.contains(&"raise_dispute") && listed.contains(&"clear_dispute");
        assert!(!both, "a raise and a clear listed together: {case:?}");
        for word in listed_sorted {
            let word = user_acts
                .iter()
                .find(|known| **known == word)
                .unwrap_or_else(|| panic!("an act no user takes is listed: {word}"));
            if !listed_somewhere.contains(word) {
                listed_somewhere.push(word);
            }
        }
    }
    listed_somewhere.sort_unstable();
    assert_eq!(
        listed_somewhere, user_acts,
        "some act is listed on no state"
    );
}

// The list tries each value of every act a user takes, as written apart from it, and nothing
// of a create or the system's Late mark.
#[test]
fn the_list_tries_each_value_of_every_user_act() {
    let now = Utc::now();
    let commission = CommissionId::mint();
    let title = TITLE.parse::<CommissionTitle>().expect("a valid title");

    for kind in CommissionActKind::VARIANTS {
        let calls: Vec<Call> = tried(*kind, commission, &title, now)
            .into_iter()
            .map(call_of)
            .collect();
        let expected = expected_calls(*kind, now);
        assert_eq!(format!("{calls:?}"), format!("{expected:?}"), "{kind:?}");
    }
}

// The list keeps one fixed order: the rename, the move, the dispute's raise and clear, then the
// direction, deadline and Delayed acts, each set before its clear.
#[tokio::test]
async fn the_owner_s_acts_come_in_the_contract_s_order() {
    let world = test_world::world().await;
    let now = Utc::now();
    let case = Case {
        step: LifecycleStep::Active,
        disputed: false,
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
        "raise_dispute",
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
        disputed: false,
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
        disputed: false,
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
