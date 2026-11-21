use std::{fmt, marker::PhantomData};

/// Leave for one write on `thing` in the role `R`. Only the den's write gate issues one; a
/// handler borrows it and can neither keep it nor hand it on.
pub struct WriteCapability<T, R> {
    thing: T,
    role: PhantomData<R>,
}

impl<T, R> WriteCapability<T, R> {
    /// The capability for `thing`, issued by the den's write gate. Inherent, not `From`, so
    /// clippy's `disallowed-methods` can ban it by path. It is `pub`: guards (those bans and a
    /// workspace scan) hold it to the gate, not its type, and only the gate may call it.
    pub fn gate_issued(thing: T) -> Self {
        Self {
            thing,
            role: PhantomData,
        }
    }

    /// The thing this capability is for. Named rather than `AsRef`: the surface is pinned to
    /// `gate_issued` and this.
    pub fn thing(&self) -> &T {
        &self.thing
    }
}

impl<T, R> fmt::Debug for WriteCapability<T, R> {
    /// Prints nothing of the thing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteCapability").finish_non_exhaustive()
    }
}

/// A thing not yet made, such as a commission's id before its row exists. A create's
/// capability is over `New<T>`, a type apart from `T`, so it unlocks nothing on an existing row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::From, derive_more::AsRef)]
pub struct New<T>(T);

/// The role of whoever makes a new thing.
pub enum Creator {}

#[cfg(test)]
mod tests;
