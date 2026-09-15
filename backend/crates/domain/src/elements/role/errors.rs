/// A stored role discriminant outside the four known roles — a schema-drift
/// signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl std::fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown role {:?}", self.0)
    }
}

impl std::error::Error for UnknownRole {}

/// A role alias that failed validation — empty after trimming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidRoleAlias;

impl std::fmt::Display for InvalidRoleAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a role alias cannot be empty")
    }
}

impl std::error::Error for InvalidRoleAlias {}
