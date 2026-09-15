use std::{ops::Deref, str::FromStr};

use crate::elements::id::{IdError, parse_uuid};

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
