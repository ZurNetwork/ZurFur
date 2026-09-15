use crate::elements::handle::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    Private,
    Public { handle: Handle },
}
