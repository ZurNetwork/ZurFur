//! The one map between the domain's [`Level`] and Atta's, each way.

use domain::elements::level::Level;

/// Atta's level for a node's own level, as the read host's `found` and the write's builder hand it over.
pub(in crate::den) fn atta_level(level: Level) -> atta::Level {
    match level {
        Level::Private => atta::Level::Private,
        Level::Listed => atta::Level::Listed,
        Level::Public => atta::Level::Public,
    }
}

/// The domain's level for the level Atta answers with.
pub(in crate::den) fn den_level(level: atta::Level) -> Level {
    match level {
        atta::Level::Private => Level::Private,
        atta::Level::Listed => Level::Listed,
        atta::Level::Public => Level::Public,
    }
}

#[cfg(test)]
mod tests;
