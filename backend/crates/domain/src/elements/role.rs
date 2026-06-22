#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Owner(Option<String>),
    Admin(Option<String>),
    Manager(Option<String>),
    Member(Option<String>),
}

/// A stored role discriminant that isn't one of the four known roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl std::fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown role {:?}", self.0)
    }
}

impl std::error::Error for UnknownRole {}

impl TryFrom<String> for Role {
    type Error = UnknownRole;

    /// Parse a stored role discriminant (`owner` | `admin` | `manager` | `member`)
    /// back into a `Role`. The parent slot is `None`: the discriminant alone can't
    /// carry it — the parent is a separate column, and is always NULL on the floor
    /// (only Owner exists). When the role tree lands, reconstruct the parent
    /// alongside, e.g. via a `TryFrom<(String, Option<String>)>`.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "owner" => Ok(Role::Owner(None)),
            "admin" => Ok(Role::Admin(None)),
            "manager" => Ok(Role::Manager(None)),
            "member" => Ok(Role::Member(None)),
            _ => Err(UnknownRole(value)),
        }
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner(_) => "owner",
            Role::Admin(_) => "admin",
            Role::Manager(_) => "manager",
            Role::Member(_) => "member",
        }
    }
}
