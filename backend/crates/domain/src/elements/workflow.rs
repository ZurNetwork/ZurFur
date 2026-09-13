//! Workflows — an account's boards: ordered columns of commission cards.
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
mod tests;
