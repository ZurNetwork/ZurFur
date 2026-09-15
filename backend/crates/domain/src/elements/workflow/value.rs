use std::str::FromStr;

use crate::string_builder::{StringBuilder, StringBuilderViolation};

/// A board's name: trimmed, never blank, at most 196 characters.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct WorkflowName(String);

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
