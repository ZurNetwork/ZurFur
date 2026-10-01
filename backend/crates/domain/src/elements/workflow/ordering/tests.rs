use super::*;
use crate::elements::{account::AccountId, commission::Visibility, did::Did};

// ---- columns -----------------------------------------------------------

fn account() -> AccountId {
    AccountId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())))
}

fn empty_workflow() -> Workflow {
    let name = "Board".parse().expect("a valid name");
    Workflow::new(name, account(), Visibility::Private)
}

/// A column for `workflow`, ready to `insert`, which assigns its key.
fn column(workflow: &Workflow, name: &str) -> Column {
    let name = name.parse().expect("a valid name");
    workflow.new_column(name, Visibility::Private)
}

fn names(workflow: &Workflow) -> Vec<&str> {
    workflow.iter().map(|c| c.name.as_ref()).collect()
}

fn positions_ascend(workflow: &Workflow) -> bool {
    let keys: Vec<&Position> = workflow.iter().map(|c| &c.position).collect();
    keys.windows(2).all(|pair| pair[0] < pair[1])
}

#[test]
fn insert_places_the_column_on_this_board_at_the_index() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id;
    workflow.push(column(&workflow, "Open")).expect("room");
    workflow.push(column(&workflow, "Done")).expect("room");

    let placed = workflow
        .insert(1, column(&workflow, "Inking"))
        .expect("room in the middle");

    assert_eq!(placed.name.as_ref(), "Inking");
    assert_eq!(placed.workflow_id, workflow_id);
    assert_eq!(names(&workflow), ["Open", "Inking", "Done"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn push_and_unshift_take_the_ends_and_keep_positions_ascending() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "B")).expect("room");
    workflow.push(column(&workflow, "C")).expect("room");
    workflow.unshift(column(&workflow, "A")).expect("room");

    assert_eq!(names(&workflow), ["A", "B", "C"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn insert_refuses_an_index_past_the_end() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "Open")).expect("room");

    let refused = workflow.insert(2, column(&workflow, "Done"));

    assert_eq!(refused.err(), Some(WorkflowError::IndexOutOfRange(2)));
    assert_eq!(names(&workflow), ["Open"]);
}

#[test]
fn insert_refuses_a_name_already_on_the_board() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "Open")).expect("room");

    let refused = workflow.push(column(&workflow, "Open"));

    assert_eq!(refused.err(), Some(WorkflowError::DuplicateColumnName));
    assert_eq!(workflow.len(), 1);
}

#[test]
fn insert_refuses_a_full_board() {
    let mut workflow = empty_workflow();
    for i in 0..MAX_COLUMNS_PER_WORKFLOW {
        workflow
            .push(column(&workflow, &format!("Column {i}")))
            .expect("under the cap");
    }

    let refused = workflow.push(column(&workflow, "One too many"));

    assert_eq!(refused.err(), Some(WorkflowError::TooManyColumns));
    assert_eq!(workflow.len(), MAX_COLUMNS_PER_WORKFLOW);
}

#[test]
fn remove_element_hands_the_column_back_and_closes_the_gap() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "Open")).expect("room");
    let inking = workflow
        .push(column(&workflow, "Inking"))
        .expect("room")
        .clone();
    workflow.push(column(&workflow, "Done")).expect("room");

    let removed = workflow
        .remove_element(inking.clone())
        .expect("on the board");

    assert_eq!(removed.id, inking.id);
    assert_eq!(removed.name.as_ref(), "Inking");
    assert_eq!(names(&workflow), ["Open", "Done"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn remove_element_may_empty_the_board() {
    let mut workflow = empty_workflow();
    let only = workflow
        .push(column(&workflow, "Open"))
        .expect("room")
        .clone();

    workflow.remove_element(only).expect("on the board");

    assert!(workflow.is_empty());
}

#[test]
fn remove_element_refuses_an_unknown_id() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "Open")).expect("room");
    let stranger = column(&workflow, "Stranger");

    let refused = workflow.remove_element(stranger);

    assert_eq!(refused.err(), Some(WorkflowError::ElementNotFound));
    assert_eq!(names(&workflow), ["Open"]);
}

#[test]
fn remove_by_index_answers_none_past_the_end_even_on_an_empty_board() {
    let mut workflow = empty_workflow();
    assert_eq!(workflow.remove(0), Ok(None));

    workflow.push(column(&workflow, "Open")).expect("room");

    assert_eq!(workflow.remove(1), Ok(None));
    let removed = workflow.remove(0).expect("in range");
    assert_eq!(removed.map(|c| c.name.to_string()), Some("Open".to_owned()));
    assert!(workflow.is_empty());
}

#[test]
fn a_removed_columns_name_is_free_again() {
    let mut workflow = empty_workflow();
    workflow.push(column(&workflow, "Open")).expect("room");
    let done = workflow
        .push(column(&workflow, "Done"))
        .expect("room")
        .clone();

    workflow.remove_element(done).expect("on the board");
    workflow.unshift(column(&workflow, "Done")).expect("room");

    assert_eq!(names(&workflow), ["Done", "Open"]);
    assert!(positions_ascend(&workflow));
}

// ---- relocate ----------------------------------------------------------

fn board(columns: &[&str]) -> Workflow {
    let mut workflow = empty_workflow();
    for name in columns {
        workflow.push(column(&workflow, name)).expect("room");
    }
    workflow
}

fn keys_by_name(workflow: &Workflow) -> Vec<(String, Position)> {
    workflow
        .iter()
        .map(|c| (c.name.to_string(), c.position.clone()))
        .collect()
}

#[test]
fn relocate_forward_lands_in_front_of_the_target() {
    let mut workflow = board(&["A", "B", "C", "D", "E", "F", "G"]);
    let keys_before = keys_by_name(&workflow);

    workflow.relocate(1, 5).expect("both in range");

    assert_eq!(names(&workflow), ["A", "C", "D", "E", "B", "F", "G"]);
    assert!(positions_ascend(&workflow));
    let unmoved_keys_kept = keys_by_name(&workflow)
        .iter()
        .filter(|(name, _)| name != "B")
        .all(|pair| keys_before.contains(pair));
    assert!(unmoved_keys_kept, "only the moved column is re-keyed");
}

#[test]
fn relocate_backward_lands_in_front_of_the_target() {
    let mut workflow = board(&["A", "B", "C", "D", "E", "F", "G"]);

    workflow.relocate(5, 1).expect("both in range");

    assert_eq!(names(&workflow), ["A", "F", "B", "C", "D", "E", "G"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn relocate_past_the_end_lands_last() {
    let mut workflow = board(&["A", "B", "C"]);

    workflow.relocate(0, 99).expect("from is in range");

    assert_eq!(names(&workflow), ["B", "C", "A"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn relocate_onto_its_own_spot_changes_nothing() {
    let mut workflow = board(&["A", "B", "C"]);

    workflow.relocate(1, 1).expect("in range");
    assert_eq!(names(&workflow), ["A", "B", "C"]);

    workflow.relocate(1, 2).expect("in range");
    assert_eq!(names(&workflow), ["A", "B", "C"]);
    assert!(positions_ascend(&workflow));
}

#[test]
fn relocate_refuses_a_from_past_the_end() {
    let mut workflow = board(&["A", "B"]);

    let refused = workflow.relocate(2, 0);

    assert_eq!(refused, Err(WorkflowError::IndexOutOfRange(2)));
    assert_eq!(names(&workflow), ["A", "B"]);
    assert_eq!(
        empty_workflow().relocate(0, 0),
        Err(WorkflowError::IndexOutOfRange(0))
    );
}

// ---- cards -------------------------------------------------------------

fn empty_column() -> Column {
    column(&empty_workflow(), "Open")
}

fn commission() -> CommissionId {
    CommissionId::from(uuid::Uuid::now_v7())
}

fn cards(column: &Column) -> Vec<CommissionId> {
    column.iter().copied().collect()
}

#[test]
fn insert_places_the_card_at_the_index() {
    let mut column = empty_column();
    let (a, b, c) = (commission(), commission(), commission());
    column.push(a).expect("room");
    column.push(b).expect("room");

    let placed = column.insert(1, c).expect("room in the middle");

    assert_eq!(*placed, c);
    assert_eq!(cards(&column), [a, c, b]);
}

#[test]
fn push_and_unshift_take_the_ends() {
    let mut column = empty_column();
    let (a, b, c) = (commission(), commission(), commission());
    column.push(b).expect("room");
    column.push(c).expect("room");
    column.unshift(a).expect("room");

    assert_eq!(cards(&column), [a, b, c]);
}

#[test]
fn insert_refuses_a_card_already_in_the_column() {
    let mut column = empty_column();
    let a = commission();
    column.push(a).expect("room");

    let refused = column.unshift(a);

    assert_eq!(refused.err(), Some(WorkflowError::DuplicateCommission));
    assert_eq!(cards(&column), [a]);
}

#[test]
fn insert_refuses_a_card_index_past_the_end() {
    let mut column = empty_column();
    column.push(commission()).expect("room");

    let refused = column.insert(2, commission());

    assert_eq!(refused.err(), Some(WorkflowError::IndexOutOfRange(2)));
    assert_eq!(column.len(), 1);
}

#[test]
fn remove_element_takes_the_card_out_and_refuses_a_stranger() {
    let mut column = empty_column();
    let (a, b) = (commission(), commission());
    column.push(a).expect("room");
    column.push(b).expect("room");

    assert_eq!(column.remove_element(a), Ok(a));
    assert_eq!(cards(&column), [b]);
    assert_eq!(
        column.remove_element(a),
        Err(WorkflowError::ElementNotFound)
    );
}

#[test]
fn remove_by_index_answers_none_past_the_end_even_on_an_empty_column() {
    let mut column = empty_column();
    assert_eq!(column.remove(0), Ok(None));

    let a = commission();
    column.push(a).expect("room");

    assert_eq!(column.remove(1), Ok(None));
    assert_eq!(column.remove(0), Ok(Some(a)));
    assert_eq!(column.pop(), Ok(None));
}

#[test]
fn relocate_moves_the_card_in_front_of_the_target() {
    let mut column = empty_column();
    let deck: Vec<CommissionId> = (0..5).map(|_| commission()).collect();
    for card in &deck {
        column.push(*card).expect("room");
    }
    let (a, b, c, d, e) = (deck[0], deck[1], deck[2], deck[3], deck[4]);

    column.relocate(1, 4).expect("in range");
    assert_eq!(cards(&column), [a, c, d, b, e]);

    column.relocate(4, 0).expect("in range");
    assert_eq!(cards(&column), [e, a, c, d, b]);

    column.relocate(0, 99).expect("from is in range");
    assert_eq!(cards(&column), [a, c, d, b, e]);

    assert_eq!(
        column.relocate(5, 0),
        Err(WorkflowError::IndexOutOfRange(5))
    );
}
