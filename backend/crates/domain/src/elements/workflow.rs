//! Workflows — an account's boards: ordered columns of commission cards.
//! (DESIGN 9895957)
//!
//! A Workflow knows about commissions, never the reverse. Columns and cards are
//! ordered by [`Position`], a base-62 fractional key compared bytewise, so an
//! insert mints a key between its neighbours and nothing is renumbered — a store
//! must order it bytewise too (`text COLLATE "C"`).

use std::{ops::Deref, str::FromStr};

pub const MAX_COLUMNS_PER_WORKFLOW: usize = 50;

use crate::{
    elements::{
        account::AccountId,
        commission::{CommissionId, Visibility},
        id::{IdError, parse_uuid},
    },
    string_builder::StringBuilder,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowId(uuid::Uuid);

impl Deref for WorkflowId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<uuid::Uuid> for WorkflowId {
    fn from(value: uuid::Uuid) -> Self {
        Self(value)
    }
}
impl FromStr for WorkflowId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = parse_uuid(s).map_err(|_| IdError::ParsingError)?;
        Ok(Self(uuid))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnId(uuid::Uuid);

impl Deref for ColumnId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<uuid::Uuid> for ColumnId {
    fn from(value: uuid::Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for ColumnId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(parse_uuid(s).map_err(|_| IdError::ParsingError)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowName(String);
impl Deref for WorkflowName {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for WorkflowName {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = StringBuilder::new(s)
            .trimmed()
            .non_empty()
            .max_chars(196)
            .build()
            .map_err(|_| IdError::ParsingError)?;

        Ok(Self(result))
    }
}

/// Why a [`Workflow`] refused a column mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    /// The board already holds [`MAX_COLUMNS_PER_WORKFLOW`] columns.
    TooManyColumns,
    /// A column of that exact name is already on the board.
    DuplicateColumnName,
    /// The commission is already in the column.
    DuplicateCommission,
    /// The insert index is past the end. Carries the offending index.
    IndexOutOfRange(usize),
    /// No element with that identity is in the list.
    ElementNotFound,
    /// Stored column keys do not strictly ascend.
    KeysOutOfOrder,
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::TooManyColumns => {
                write!(
                    f,
                    "a workflow holds at most {MAX_COLUMNS_PER_WORKFLOW} columns"
                )
            }
            WorkflowError::DuplicateColumnName => {
                write!(f, "a column with that name is already on this workflow")
            }
            WorkflowError::DuplicateCommission => {
                write!(f, "that commission is already in this column")
            }
            WorkflowError::IndexOutOfRange(index) => {
                write!(f, "index {index} is past the end of the list")
            }
            WorkflowError::ElementNotFound => write!(f, "no such resource"),
            WorkflowError::KeysOutOfOrder => write!(f, "stored column keys are out of order"),
        }
    }
}

impl std::error::Error for WorkflowError {}

/// Digits of a [`Position`], in byte order — so `Ord` on the string is `Ord` on the key.
const POSITION_DIGITS: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const POSITION_BASE: usize = POSITION_DIGITS.len();

/// A column's place among its siblings: a base-62 fractional key compared
/// bytewise. Inserting mints a key *between* two neighbours; nothing is ever
/// renumbered. A store must order it bytewise too (`text COLLATE "C"`).
///
/// Invariant: non-empty, alphabet digits only, never ends in `0` — that is
/// what guarantees [`between`](Position::between) always has room.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position(String);

/// Why a string was rejected as a [`Position`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PositionError {
    Empty,
    /// A character outside `[0-9A-Za-z]`. Carries the offending char.
    InvalidDigit(char),
    /// Ends in `0`, which denotes the same key as without it.
    TrailingZero,
}

impl std::fmt::Display for PositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PositionError::Empty => write!(f, "position must not be empty"),
            PositionError::InvalidDigit(c) => write!(
                f,
                "position contains an invalid character {c:?}; only 0-9, A-Z and a-z are allowed"
            ),
            PositionError::TrailingZero => write!(f, "position must not end in '0'"),
        }
    }
}

impl std::error::Error for PositionError {}

impl Position {
    /// A key strictly between `lo` and `hi`; `None` stands for the start
    /// (`lo`) or the end (`hi`) of the sequence.
    ///
    /// # Panics
    /// If `lo` does not sort before `hi` — a caller bug, since neighbours come
    /// from an already-ordered sequence.
    pub fn between(lo: Option<&Position>, hi: Option<&Position>) -> Position {
        let lo = lo.map_or(&[][..], |p| p.0.as_bytes());
        let hi = hi.map(|p| p.0.as_bytes());
        assert!(hi.is_none_or(|hi| lo < hi), "lo must sort before hi");

        let mut out = Vec::new();
        midpoint(lo, hi, &mut out);
        Position(String::from_utf8(out).expect("the alphabet is ASCII"))
    }
}

/// Appends a key strictly between `lo` and `hi` to `out`; `hi == None` is +∞.
fn midpoint(mut lo: &[u8], mut hi: Option<&[u8]>, out: &mut Vec<u8>) {
    loop {
        if let Some(h) = hi {
            let shared = shared_prefix(lo, h);
            if shared > 0 {
                out.extend_from_slice(&h[..shared]);
                lo = lo.get(shared..).unwrap_or(&[]);
                hi = Some(&h[shared..]);
                continue;
            }
        }

        let digit_lo = lo.first().map_or(0, |d| digit_index(*d));
        let digit_hi = hi
            .and_then(|h| h.first())
            .map_or(POSITION_BASE, |d| digit_index(*d));

        if digit_hi - digit_lo > 1 {
            out.push(POSITION_DIGITS[(digit_lo + digit_hi).div_ceil(2)]);
            return;
        }

        // The digits are adjacent: no room at this place.
        match hi {
            // `hi` continues past this digit, so its first digit alone sits below it.
            Some(h) if h.len() > 1 => {
                out.push(h[0]);
                return;
            }
            // Otherwise step into `lo`'s next place with nothing above it.
            _ => {
                out.push(POSITION_DIGITS[digit_lo]);
                lo = lo.get(1..).unwrap_or(&[]);
                hi = None;
            }
        }
    }
}

/// Length of the prefix `lo` and `hi` share; `lo` is read as zero-padded.
fn shared_prefix(lo: &[u8], hi: &[u8]) -> usize {
    hi.iter()
        .enumerate()
        .take_while(|(i, h)| lo.get(*i).copied().unwrap_or(POSITION_DIGITS[0]) == **h)
        .count()
}

fn digit_index(digit: u8) -> usize {
    POSITION_DIGITS
        .iter()
        .position(|d| *d == digit)
        .expect("a Position holds only alphabet digits")
}

impl FromStr for Position {
    type Err = PositionError;

    /// Validate a stored key against the [`Position`] invariant.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(PositionError::Empty);
        }
        if let Some(c) = s.chars().find(|c| !c.is_ascii_alphanumeric()) {
            return Err(PositionError::InvalidDigit(c));
        }
        if s.ends_with('0') {
            return Err(PositionError::TrailingZero);
        }
        Ok(Self(s.to_owned()))
    }
}

impl AsRef<str> for Position {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Workflow {
    pub id: WorkflowId,
    pub name: WorkflowName,
    pub account_id: AccountId,
    /// Sorted by [`Column::position`]. Private so only the [`LexOrdering`]
    /// methods, which hold the cap, the unique names and the order, touch it.
    columns: Vec<Column>,
    pub visibility: Visibility,
}

impl Workflow {
    /// A fresh, empty board with a minted id.
    pub fn new(name: WorkflowName, account_id: AccountId, visibility: Visibility) -> Self {
        Workflow {
            id: WorkflowId::from(uuid::Uuid::now_v7()),
            name,
            account_id,
            columns: Vec::new(),
            visibility,
        }
    }

    /// A board rehydrated from stored rows. The one door for an unchecked
    /// list: refuses what [`insert`](LexOrdering::insert) would refuse, plus
    /// keys that do not strictly ascend.
    pub fn loaded(
        id: WorkflowId,
        name: WorkflowName,
        account_id: AccountId,
        visibility: Visibility,
        columns: Vec<Column>,
    ) -> Result<Self, WorkflowError> {
        if columns.len() > MAX_COLUMNS_PER_WORKFLOW {
            return Err(WorkflowError::TooManyColumns);
        }
        let names_unique = columns.iter().enumerate().all(|(i, column)| {
            columns[..i]
                .iter()
                .all(|earlier| earlier.name != column.name)
        });
        if !names_unique {
            return Err(WorkflowError::DuplicateColumnName);
        }
        let keys_ascend = columns
            .windows(2)
            .all(|pair| pair[0].position < pair[1].position);
        if !keys_ascend {
            return Err(WorkflowError::KeysOutOfOrder);
        }

        Ok(Workflow {
            id,
            name,
            account_id,
            columns,
            visibility,
        })
    }

    /// A column for this board, not yet on it: [`insert`](LexOrdering::insert)
    /// assigns its key.
    pub fn new_column(&self, name: ColumnName, visibility: Visibility) -> Column {
        Column {
            id: ColumnId::from(uuid::Uuid::now_v7()),
            workflow_id: self.id.clone(),
            name,
            visibility,
            position: Position::between(None, None),
            commissions: Vec::new(),
        }
    }

    pub fn rename_column(
        &mut self,
        column_id: &ColumnId,
        name: ColumnName,
    ) -> Result<&Column, WorkflowError> {
        if self
            .columns
            .iter()
            .any(|c| c.name == name && c.id != *column_id)
        {
            return Err(WorkflowError::DuplicateColumnName);
        }

        let column = self
            .columns
            .iter_mut()
            .find(|c| c.id == *column_id)
            .ok_or(WorkflowError::ElementNotFound)?;

        column.name = name;
        Ok(column)
    }
}

pub type ColumnName = WorkflowName;

/// A column on a board. Its own id lets the artist set its visibility apart
/// from its board's.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub id: ColumnId,
    pub workflow_id: WorkflowId,
    pub name: ColumnName,
    pub visibility: Visibility,
    /// Its key among the siblings on the board.
    pub position: Position,
    /// The cards in board order. Private so a card can only enter once.
    commissions: Vec<CommissionId>,
}

impl Column {
    /// A column rehydrated from stored rows. Refuses a card listed twice.
    pub fn loaded(
        id: ColumnId,
        workflow_id: WorkflowId,
        name: ColumnName,
        visibility: Visibility,
        position: Position,
        commissions: Vec<CommissionId>,
    ) -> Result<Self, WorkflowError> {
        let cards_unique = commissions
            .iter()
            .enumerate()
            .all(|(i, card)| !commissions[..i].contains(card));
        if !cards_unique {
            return Err(WorkflowError::DuplicateCommission);
        }

        Ok(Column {
            id,
            workflow_id,
            name,
            visibility,
            position,
            commissions,
        })
    }
}

pub trait LexOrdering {
    type Element;
    type Err;

    /// The elements in order, borrowed.
    fn iter(&self) -> impl Iterator<Item = &Self::Element>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get(&self, index: usize) -> Option<&Self::Element>;

    fn insert(&mut self, index: usize, element: Self::Element)
    -> Result<&Self::Element, Self::Err>;
    fn remove(&mut self, index: usize) -> Result<Option<Self::Element>, Self::Err>;
    fn remove_element(&mut self, element: Self::Element) -> Result<Self::Element, Self::Err>;
    fn push(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err>;
    fn unshift(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err>;
    fn pop(&mut self) -> Result<Option<Self::Element>, Self::Err>;
    fn relocate(&mut self, from_index: usize, to_index: usize) -> Result<(), Self::Err>;
}

impl LexOrdering for Workflow {
    type Element = Column;
    type Err = WorkflowError;

    fn iter(&self) -> impl Iterator<Item = &Self::Element> {
        self.columns.iter()
    }

    fn len(&self) -> usize {
        self.columns.len()
    }

    fn get(&self, index: usize) -> Option<&Self::Element> {
        self.columns.get(index)
    }

    /// Places the column at `index`, keying it between its neighbours. The
    /// column keeps its id, name, visibility and cards; only its board and
    /// key are assigned here.
    fn insert(
        &mut self,
        index: usize,
        mut element: Self::Element,
    ) -> Result<&Self::Element, Self::Err> {
        if self.columns.len() >= MAX_COLUMNS_PER_WORKFLOW {
            return Err(WorkflowError::TooManyColumns);
        }
        if self
            .columns
            .iter()
            .any(|sibling| sibling.name == element.name)
        {
            return Err(WorkflowError::DuplicateColumnName);
        }
        if index > self.columns.len() {
            return Err(WorkflowError::IndexOutOfRange(index));
        }

        let before = index
            .checked_sub(1)
            .and_then(|i| self.columns.get(i))
            .map(|sibling| &sibling.position);
        let after = self.columns.get(index).map(|sibling| &sibling.position);
        let position = Position::between(before, after);

        element.workflow_id = self.id.clone();
        element.position = position;
        self.columns.insert(index, element);
        Ok(&self.columns[index])
    }

    fn remove_element(&mut self, element: Self::Element) -> Result<Self::Element, Self::Err> {
        let at = self
            .columns
            .iter()
            .position(|column| column.id == element.id)
            .ok_or(WorkflowError::ElementNotFound)?;
        let found = self.columns.remove(at);
        Ok(found)
    }

    fn remove(&mut self, index: usize) -> Result<Option<Self::Element>, Self::Err> {
        if index >= self.columns.len() {
            return Ok(None);
        }
        let removed = self.columns.remove(index);
        Ok(Some(removed))
    }

    fn push(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err> {
        let current_length = self.columns.len();
        self.insert(current_length, element)
    }

    fn unshift(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err> {
        self.insert(0, element)
    }

    fn pop(&mut self) -> Result<Option<Self::Element>, Self::Err> {
        Ok(self.columns.pop())
    }

    /// Only the moved column is re-keyed, between its new neighbours.
    fn relocate(&mut self, from_index: usize, to_index: usize) -> Result<(), Self::Err> {
        let landed = relocate_within(&mut self.columns, from_index, to_index)?;

        let before = landed
            .checked_sub(1)
            .and_then(|i| self.columns.get(i))
            .map(|sibling| &sibling.position);
        let after = self
            .columns
            .get(landed + 1)
            .map(|sibling| &sibling.position);
        let position = Position::between(before, after);

        self.columns[landed].position = position;
        Ok(())
    }
}

/// Moves `items[from]` in front of `items[to]` (`to` past the end means last)
/// by rotating the span between them; answers the index it landed on.
fn relocate_within<T>(items: &mut [T], from: usize, to: usize) -> Result<usize, WorkflowError> {
    let len = items.len();
    if from >= len {
        return Err(WorkflowError::IndexOutOfRange(from));
    }
    let to = to.min(len);

    if from < to {
        items[from..to].rotate_left(1);
        Ok(to - 1)
    } else {
        items[to..=from].rotate_right(1);
        Ok(to)
    }
}

impl LexOrdering for Column {
    type Element = CommissionId;

    type Err = WorkflowError;

    fn iter(&self) -> impl Iterator<Item = &Self::Element> {
        self.commissions.iter()
    }

    fn len(&self) -> usize {
        self.commissions.len()
    }

    fn get(&self, index: usize) -> Option<&Self::Element> {
        self.commissions.get(index)
    }

    /// Places the commission at `index` (0 = first, `commissions.len()` =
    /// last). Refuses a commission already in the column or an index past the
    /// end.
    fn insert(
        &mut self,
        index: usize,
        element: Self::Element,
    ) -> Result<&Self::Element, Self::Err> {
        if self.commissions.contains(&element) {
            return Err(WorkflowError::DuplicateCommission);
        }
        if index > self.commissions.len() {
            return Err(WorkflowError::IndexOutOfRange(index));
        }

        self.commissions.insert(index, element);
        Ok(&self.commissions[index])
    }

    fn remove(&mut self, index: usize) -> Result<Option<Self::Element>, Self::Err> {
        if index >= self.commissions.len() {
            return Ok(None);
        }
        let removed = self.commissions.remove(index);
        Ok(Some(removed))
    }

    fn remove_element(&mut self, element: Self::Element) -> Result<Self::Element, Self::Err> {
        let index = self
            .commissions
            .iter()
            .position(|c| *c == element)
            .ok_or(WorkflowError::ElementNotFound)?;

        let element = self.commissions.remove(index);

        Ok(element)
    }

    fn push(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err> {
        let current_length = self.commissions.len();
        self.insert(current_length, element)
    }

    fn unshift(&mut self, element: Self::Element) -> Result<&Self::Element, Self::Err> {
        self.insert(0, element)
    }

    fn pop(&mut self) -> Result<Option<Self::Element>, Self::Err> {
        Ok(self.commissions.pop())
    }

    /// Cards carry no key of their own: the rotation is the whole move.
    fn relocate(&mut self, from_index: usize, to_index: usize) -> Result<(), Self::Err> {
        relocate_within(&mut self.commissions, from_index, to_index).map(|_landed| ())
    }
}

/// `for column in &workflow`, borrowing, in board order.
impl<'a> IntoIterator for &'a Workflow {
    type Item = &'a Column;
    type IntoIter = std::slice::Iter<'a, Column>;

    fn into_iter(self) -> Self::IntoIter {
        self.columns.iter()
    }
}

/// `for column in workflow`, consuming the board for its columns.
impl IntoIterator for Workflow {
    type Item = Column;
    type IntoIter = std::vec::IntoIter<Column>;

    fn into_iter(self) -> Self::IntoIter {
        self.columns.into_iter()
    }
}

/// `for card in &column`, borrowing, in board order.
impl<'a> IntoIterator for &'a Column {
    type Item = &'a CommissionId;
    type IntoIter = std::slice::Iter<'a, CommissionId>;

    fn into_iter(self) -> Self::IntoIter {
        self.commissions.iter()
    }
}

/// `for card in column`, consuming the column for its cards.
impl IntoIterator for Column {
    type Item = CommissionId;
    type IntoIter = std::vec::IntoIter<CommissionId>;

    fn into_iter(self) -> Self::IntoIter {
        self.commissions.into_iter()
    }
}

#[cfg(test)]
mod tests {
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
        AccountId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())))
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
}
