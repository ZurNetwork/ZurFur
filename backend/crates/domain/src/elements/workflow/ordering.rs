use crate::elements::commission::CommissionId;

use super::{
    entity::{Column, MAX_COLUMNS_PER_WORKFLOW, Workflow},
    errors::WorkflowError,
    position::Position,
};

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

        element.workflow_id = self.id;
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
