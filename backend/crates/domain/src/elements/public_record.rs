//! Public-boundary record value types — the domain's protocol-free vocabulary
//! for the records Zurfur publishes into an actor's atproto repo.
//!
//! These are what the [`PublicRecords`](crate::ports::PublicRecords) port speaks
//! in: they mirror the `app.zurfur.feed.post` lexicon as domain data and carry
//! no AT-Protocol types — the wire shape, CBOR and CID computation live in
//! `adapter-atproto`.

mod at_uri;
mod errors;
mod feed_post;
mod keys;
mod record;
mod refs;

pub use at_uri::AtUri;
pub use errors::AtUriParseError;
pub use feed_post::{AspectRatio, Credit, Embed, FeedPost, ReplyRef, ReplySubject, SelfLabels};
pub use keys::{Nsid, RecordKey};
pub use record::PublicRecord;
pub use refs::{BlobRef, RecordRef, StrongRef};
