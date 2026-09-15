use crate::elements::{
    account::AccountId,
    commission::{CommissionId, Visibility},
};

use super::{
    errors::WorkflowError,
    id::{ColumnId, WorkflowId},
    position::Position,
    value::{ColumnName, WorkflowName},
};

pub const MAX_COLUMNS_PER_WORKFLOW: usize = 50;

pub struct Workflow {
    pub id: WorkflowId,
    pub name: WorkflowName,
    pub account_id: AccountId,
    /// Sorted by [`Column::position`]. Private so only the [`LexOrdering`]
    /// methods, which hold the cap, the unique names and the order, touch it.
    pub(super) columns: Vec<Column>,
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
    pub(super) commissions: Vec<CommissionId>,
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

#[cfg(test)]
mod tests;
