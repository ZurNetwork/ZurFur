use std::ops::Deref;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Did(String);

impl Deref for Did {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
