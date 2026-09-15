use std::{ops::Deref, str::FromStr};

use crate::{elements::id::IdError, string_builder::StringBuilder};

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

pub type ColumnName = WorkflowName;
