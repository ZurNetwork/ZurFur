use super::super::ordering::LexOrdering;
use super::*;
use crate::elements::did::Did;

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

fn board(columns: &[&str]) -> Workflow {
    let mut workflow = empty_workflow();
    for name in columns {
        workflow.push(column(&workflow, name)).expect("room");
    }
    workflow
}

fn empty_column() -> Column {
    column(&empty_workflow(), "Open")
}

fn commission() -> CommissionId {
    CommissionId::from(uuid::Uuid::now_v7())
}

#[test]
fn insert_gives_each_column_its_own_id_and_settings() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id;

    let first = workflow.push(column(&workflow, "Open")).expect("room");
    assert_eq!(first.workflow_id, workflow_id);
    assert_eq!(first.visibility, Visibility::Private);
    let first_id = first.id;

    let second_id = workflow.push(column(&workflow, "Done")).expect("room").id;

    assert_ne!(second_id, first_id);
}

// ---- identity & loading ------------------------------------------------

#[test]
fn insert_keeps_the_columns_id_and_own_visibility() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id;
    let name = "Showcase".parse().expect("a valid name");
    let column = workflow.new_column(name, Visibility::Public);
    let id = column.id;

    let placed = workflow.push(column).expect("room");

    assert_eq!(placed.id, id);
    assert_eq!(placed.visibility, Visibility::Public);
    assert_eq!(placed.workflow_id, workflow_id);
}

#[test]
fn remove_then_insert_keeps_the_columns_identity() {
    let mut workflow = board(&["A", "B", "C"]);
    let b = workflow.get(1).expect("on the board").clone();

    let taken = workflow.remove_element(b.clone()).expect("on the board");
    workflow.unshift(taken).expect("room");

    assert_eq!(names(&workflow), ["B", "A", "C"]);
    assert_eq!(workflow.get(0).map(|c| &c.id), Some(&b.id));
    assert!(positions_ascend(&workflow));
}

#[test]
fn boards_and_columns_iterate_by_reference_and_by_value() {
    let mut workflow = board(&["A", "B"]);
    let (a, b) = (commission(), commission());
    let first = workflow.get(0).expect("on the board").clone();
    let mut column = workflow.remove_element(first).expect("on the board");
    column.push(a).expect("room");
    column.push(b).expect("room");

    let mut seen = Vec::new();
    for column in &workflow {
        seen.push(column.name.as_ref());
    }
    assert_eq!(seen, ["B"]);

    let mut cards_seen = Vec::new();
    for card in &column {
        cards_seen.push(*card);
    }
    assert_eq!(cards_seen, [a, b]);

    let owned: Vec<CommissionId> = column.into_iter().collect();
    assert_eq!(owned, [a, b]);
}

#[test]
fn loaded_accepts_sorted_rows_and_refuses_out_of_order_keys() {
    let built = board(&["A", "B", "C"]);
    let rows: Vec<Column> = built.iter().cloned().collect();
    let name = || built.name.clone();

    let loaded = Workflow::loaded(
        built.id,
        name(),
        built.account_id.clone(),
        Visibility::Private,
        rows.clone(),
    )
    .expect("sorted rows load");
    assert_eq!(names(&loaded), ["A", "B", "C"]);

    let mut shuffled = rows;
    shuffled.swap(0, 2);
    let refused = Workflow::loaded(
        built.id,
        name(),
        built.account_id.clone(),
        Visibility::Private,
        shuffled,
    );
    assert_eq!(refused.err(), Some(WorkflowError::KeysOutOfOrder));
}

#[test]
fn loaded_refuses_a_duplicate_column_name_and_a_full_board() {
    let built = board(&["A", "B"]);
    let mut twice: Vec<Column> = built.iter().cloned().collect();
    let mut duplicate = twice[0].clone();
    duplicate.id = ColumnId::from(uuid::Uuid::now_v7());
    duplicate.position = Position::between(twice.last().map(|c| &c.position), None);
    twice.push(duplicate);

    let refused = Workflow::loaded(
        built.id,
        built.name.clone(),
        built.account_id.clone(),
        Visibility::Private,
        twice,
    );
    assert_eq!(refused.err(), Some(WorkflowError::DuplicateColumnName));

    let full = (0..=MAX_COLUMNS_PER_WORKFLOW)
        .map(|i| format!("Column {i}"))
        .collect::<Vec<_>>();
    let over_cap: Vec<&str> = full.iter().map(String::as_str).collect();
    let mut rows = Vec::new();
    for name in over_cap {
        let column = column(&built, name);
        rows.push(column);
    }
    let refused = Workflow::loaded(
        built.id,
        built.name.clone(),
        built.account_id.clone(),
        Visibility::Private,
        rows,
    );
    assert_eq!(refused.err(), Some(WorkflowError::TooManyColumns));
}

#[test]
fn column_loaded_refuses_a_card_listed_twice() {
    let template = empty_column();
    let a = commission();

    let refused = Column::loaded(
        template.id,
        template.workflow_id,
        template.name.clone(),
        Visibility::Private,
        template.position.clone(),
        vec![a, commission(), a],
    );

    assert_eq!(refused.err(), Some(WorkflowError::DuplicateCommission));
}
