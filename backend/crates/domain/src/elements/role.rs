pub enum Role {
    Owner(Option<String>),
    Admin(Option<String>),
    Manager(Option<String>),
    Member(Option<String>),
}
