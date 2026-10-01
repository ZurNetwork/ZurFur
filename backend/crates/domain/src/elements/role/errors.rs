/// A stored role discriminant outside the four known roles — a schema-drift
/// signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown role {0:?}")]
pub struct UnknownRole(pub String);

/// A role alias that failed validation — empty after trimming.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a role alias cannot be empty")]
pub struct InvalidRoleAlias;
