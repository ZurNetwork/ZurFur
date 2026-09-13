use super::*;
use crate::elements::did::Did;

fn position(key: &str) -> Position {
    key.parse().expect("test keys hold the invariant")
}

/// Mints the key between two optional neighbours and checks the contract
/// every caller relies on: strictly between, and never ending in `0`.
fn key_between(lo: Option<&str>, hi: Option<&str>) -> String {
    let lo = lo.map(position);
    let hi = hi.map(position);
    let minted = Position::between(lo.as_ref(), hi.as_ref());
    if let Some(lo) = &lo {
        assert!(lo < &minted, "{lo} must sort before {minted}");
    }
    if let Some(hi) = &hi {
        assert!(&minted < hi, "{minted} must sort before {hi}");
    }
    assert!(!minted.as_ref().ends_with('0'), "{minted} ends in '0'");
    minted.to_string()
}

// ---- between -----------------------------------------------------------

#[test]
fn the_first_key_is_the_middle_digit() {
    assert_eq!(key_between(None, None), "V");
}

#[test]
fn before_and_after_pick_the_middle_of_the_open_side() {
    assert_eq!(key_between(None, Some("V")), "G");
    assert_eq!(key_between(Some("V"), None), "l");
}

#[test]
fn a_gap_between_digits_takes_the_middle_digit() {
    assert_eq!(key_between(Some("a"), Some("c")), "b");
    assert_eq!(key_between(Some("a"), Some("a5")), "a3");
}

#[test]
fn adjacent_digits_extend_the_lower_key() {
    assert_eq!(key_between(Some("a"), Some("b")), "aV");
    assert_eq!(key_between(Some("aV"), Some("b")), "al");
    assert_eq!(key_between(Some("a9"), Some("b")), "aa");
}

#[test]
fn the_last_digit_still_has_room_after_it() {
    assert_eq!(key_between(Some("z"), None), "zV");
}

#[test]
fn a_leading_zero_is_allowed_only_a_trailing_one_is_not() {
    assert_eq!(key_between(None, Some("1")), "0V");
    assert_eq!(key_between(None, Some("05")), "03");
}

#[test]
fn keys_stay_ordered_under_repeated_insertion_everywhere() {
    let mut keys = vec![position("V")];
    for _ in 0..200 {
        let front = Position::between(None, keys.first());
        keys.insert(0, front);
    }
    for _ in 0..200 {
        let back = Position::between(keys.last(), None);
        keys.push(back);
    }
    for i in 0..keys.len() - 1 {
        let middle = Position::between(Some(&keys[i]), Some(&keys[i + 1]));
        keys.insert(i + 1, middle);
    }

    let all_ascending = keys.windows(2).all(|pair| pair[0] < pair[1]);
    assert!(all_ascending);
    assert!(keys.iter().all(|key| !key.as_ref().ends_with('0')));
}

#[test]
#[should_panic(expected = "lo must sort before hi")]
fn between_refuses_misordered_neighbours() {
    let lo = position("b");
    let hi = position("a");
    Position::between(Some(&lo), Some(&hi));
}

// ---- ordering ----------------------------------------------------------

#[test]
fn ordering_is_bytewise_with_the_prefix_rule() {
    assert!(position("a") < position("aa"));
    assert!(position("aa") < position("b"));
    assert!(position("9") < position("A"));
    assert!(position("Z") < position("a"));
}

// ---- parse -------------------------------------------------------------

#[test]
fn parse_accepts_a_key_and_round_trips_it() {
    let parsed = "aV".parse::<Position>();
    assert_eq!(
        parsed.as_ref().map(ToString::to_string).as_deref(),
        Ok("aV")
    );
}

#[test]
fn parse_rejects_what_breaks_the_invariant() {
    assert_eq!("".parse::<Position>(), Err(PositionError::Empty));
    assert_eq!(
        "a-b".parse::<Position>(),
        Err(PositionError::InvalidDigit('-'))
    );
    assert_eq!(
        "é".parse::<Position>(),
        Err(PositionError::InvalidDigit('é'))
    );
    assert_eq!("a0".parse::<Position>(), Err(PositionError::TrailingZero));
}

// ---- columns -----------------------------------------------------------

fn account() -> AccountId {
    AccountId::new(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())))
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
    workflow.iter().map(|c| c.name.as_str()).collect()
}

fn positions_ascend(workflow: &Workflow) -> bool {
    let keys: Vec<&Position> = workflow.iter().map(|c| &c.position).collect();
    keys.windows(2).all(|pair| pair[0] < pair[1])
}

#[test]
fn insert_places_the_column_on_this_board_at_the_index() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id.clone();
    workflow.push(column(&workflow, "Open")).expect("room");
    workflow.push(column(&workflow, "Done")).expect("room");

    let placed = workflow
        .insert(1, column(&workflow, "Inking"))
        .expect("room in the middle");

    assert_eq!(placed.name.as_str(), "Inking");
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
fn insert_gives_each_column_its_own_id_and_settings() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id.clone();

    let first = workflow.push(column(&workflow, "Open")).expect("room");
    assert_eq!(first.workflow_id, workflow_id);
    assert_eq!(first.visibility, Visibility::Private);
    let first_id = first.id.clone();

    let second_id = workflow
        .push(column(&workflow, "Done"))
        .expect("room")
        .id
        .clone();

    assert_ne!(second_id, first_id);
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
    assert_eq!(removed.name.as_str(), "Inking");
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
    CommissionId::new(uuid::Uuid::now_v7())
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

// ---- identity & loading ------------------------------------------------

#[test]
fn insert_keeps_the_columns_id_and_own_visibility() {
    let mut workflow = empty_workflow();
    let workflow_id = workflow.id.clone();
    let name = "Showcase".parse().expect("a valid name");
    let column = workflow.new_column(name, Visibility::Public);
    let id = column.id.clone();

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
fn loaded_accepts_sorted_rows_and_refuses_out_of_order_keys() {
    let built = board(&["A", "B", "C"]);
    let rows: Vec<Column> = built.iter().cloned().collect();
    let name = || built.name.clone();

    let loaded = Workflow::loaded(
        built.id.clone(),
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
        built.id.clone(),
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
        built.id.clone(),
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
        built.id.clone(),
        built.name.clone(),
        built.account_id.clone(),
        Visibility::Private,
        rows,
    );
    assert_eq!(refused.err(), Some(WorkflowError::TooManyColumns));
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
        seen.push(column.name.as_str());
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
fn column_loaded_refuses_a_card_listed_twice() {
    let template = empty_column();
    let a = commission();

    let refused = Column::loaded(
        template.id.clone(),
        template.workflow_id.clone(),
        template.name.clone(),
        Visibility::Private,
        template.position.clone(),
        vec![a, commission(), a],
    );

    assert_eq!(refused.err(), Some(WorkflowError::DuplicateCommission));
}
