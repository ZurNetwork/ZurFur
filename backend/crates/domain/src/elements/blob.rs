use cid::Cid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobId(Cid);
