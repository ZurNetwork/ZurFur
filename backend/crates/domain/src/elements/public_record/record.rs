use super::{FeedPost, Nsid};

/// The extensible envelope over the public records Zurfur can publish. Variants
/// are added additively, and the variant fixes the collection NSID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRecord {
    /// An `app.zurfur.feed.post` record.
    FeedPost(FeedPost),
}

impl PublicRecord {
    /// The collection NSID this record belongs to, fixed by the variant — a
    /// caller never picks one that disagrees with the body.
    pub fn collection(&self) -> Nsid {
        match self {
            PublicRecord::FeedPost(_) => Nsid::new("app.zurfur.feed.post"),
        }
    }
}

#[cfg(test)]
mod tests;
