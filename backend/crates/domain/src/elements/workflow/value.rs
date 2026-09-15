use std::{ops::Deref, str::FromStr};

use crate::string_builder::{StringBuilder, StringBuilderViolation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowName(String);
impl Deref for WorkflowName {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for WorkflowName {
    type Err = super::WorkflowNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StringBuilder::new(s)
            .trimmed()
            .non_empty()
            .max_chars(196)
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => super::WorkflowNameError::Empty,
                StringBuilderViolation::TooLong { len, .. } => super::WorkflowNameError::TooLong(len),
                StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain never calls no_control.
                    debug_assert!(
                        false,
                        "WorkflowName's FromStr chain never calls no_control; ControlCharacter is unreachable"
                    );
                    super::WorkflowNameError::Empty
                }
            })
    }
}

pub type ColumnName = WorkflowName;
