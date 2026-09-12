//! Public-boundary record value types — the domain's protocol-free vocabulary
//! for the records Zurfur publishes into an actor's atproto repo.
//!
//! These are what the [`PublicRecords`](crate::ports::PublicRecords) port speaks
//! in: they mirror the `app.zurfur.feed.post` lexicon as domain data and carry
//! no AT-Protocol types — the wire shape, CBOR and CID computation live in
//! `adapter-atproto`. (DD 29949954)

use cid::Cid;

use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;

/// A collection NSID — the reverse-DNS name of the lexicon a record belongs to.
/// A newtype, not a validating parser; a value read off the wire is validated by
/// the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Nsid(String);

impl Nsid {
    /// Wrap an NSID string.
    pub fn new(nsid: impl Into<String>) -> Self {
        Self(nsid.into())
    }

    /// The NSID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Nsid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A record key (`rkey`) — the per-collection identifier of a single record,
/// treated as an opaque string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordKey(String);

impl RecordKey {
    /// Wrap an rkey string.
    pub fn new(rkey: impl Into<String>) -> Self {
        Self(rkey.into())
    }

    /// The rkey as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The fully-qualified address of a record in an actor's repo:
/// `at://<did>/<collection>/<rkey>` — the
/// [AT-URI](https://atproto.com/specs/at-uri-scheme) restricted to the
/// repo-record form, with no query or fragment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AtUri {
    /// The repo owner's DID (the URI authority).
    pub did: Did,
    /// The collection NSID.
    pub collection: Nsid,
    /// The record key within the collection.
    pub rkey: RecordKey,
}

/// Why an [`AtUri`] string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtUriParseError {
    /// The string did not start with the `at://` scheme.
    MissingScheme,
    /// The string did not have exactly the `authority/collection/rkey` three parts.
    Malformed,
}

impl std::fmt::Display for AtUriParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtUriParseError::MissingScheme => write!(f, "AT-URI must start with `at://`"),
            AtUriParseError::Malformed => {
                write!(f, "AT-URI must be at://<did>/<collection>/<rkey>")
            }
        }
    }
}

impl std::error::Error for AtUriParseError {}

impl AtUri {
    /// Build an AT-URI from its parts.
    pub fn new(did: Did, collection: Nsid, rkey: RecordKey) -> Self {
        Self {
            did,
            collection,
            rkey,
        }
    }

    /// Parse an `at://<did>/<collection>/<rkey>` string. None of the three
    /// parts may contain `/`, `?` or `#`, so they split unambiguously; a query
    /// or fragment is rejected outright.
    pub fn parse(s: &str) -> Result<Self, AtUriParseError> {
        let rest = s
            .strip_prefix("at://")
            .ok_or(AtUriParseError::MissingScheme)?;
        if rest.contains(['?', '#']) {
            return Err(AtUriParseError::Malformed);
        }
        let mut parts = rest.splitn(3, '/');
        let (Some(did), Some(collection), Some(rkey)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(AtUriParseError::Malformed);
        };
        if did.is_empty() || collection.is_empty() || rkey.is_empty() || rkey.contains('/') {
            return Err(AtUriParseError::Malformed);
        }
        Ok(Self {
            did: Did::new(did.to_string()),
            collection: Nsid::new(collection),
            rkey: RecordKey::new(rkey),
        })
    }
}

impl std::fmt::Display for AtUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "at://{}/{}/{}",
            self.did.as_str(),
            self.collection,
            self.rkey
        )
    }
}

impl std::str::FromStr for AtUri {
    type Err = AtUriParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A strong reference to a record: its [`AtUri`] paired with the content-hash
/// [`Cid`] of the exact revision pointed at — a pointer that a change to the
/// target invalidates. Mirrors `com.atproto.repo.strongRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongRef {
    /// The referenced record's address.
    pub uri: AtUri,
    /// The content hash of the referenced revision.
    pub cid: Cid,
}

/// The address + content hash a write returns: where the record landed and the
/// [`Cid`] of the revision just written. Distinct from [`StrongRef`] because it
/// names a write result, not a reference to another record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRef {
    /// Where the record was written.
    pub uri: AtUri,
    /// The content hash of the written revision.
    pub cid: Cid,
}

/// A reference to an uploaded blob: its content-address [`Cid`] plus the mime
/// type and byte size the repo recorded. Byte-identical blobs share a ref
/// network-wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    /// The blob's content-address.
    pub cid: Cid,
    /// The mime type the repo stored for the blob.
    pub mime_type: String,
    /// The blob's size in bytes.
    pub size: u64,
}

impl BlobRef {
    /// The blob's content-addressed identity — the
    /// [`BlobId`](crate::elements::blob::BlobId) the rest of the domain uses.
    pub fn id(&self) -> crate::elements::blob::BlobId {
        crate::elements::blob::BlobId::new(self.cid)
    }
}

/// A width:height aspect ratio hint for a media embed (both ≥ 1) — optional,
/// approximate layout metadata. Mirrors `app.zurfur.embed.media#aspectRatio`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    /// The width component (≥ 1).
    pub width: u32,
    /// The height component (≥ 1).
    pub height: u32,
}

/// A single visual-media embed: the blob, its required alt text, and an optional
/// aspect ratio. Mirrors `app.zurfur.embed.media` (the sole embed kind v1 has).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embed {
    /// The embedded media blob.
    pub blob: BlobRef,
    /// Required alt-text description; the non-blank rule is enforced at the
    /// compose/publish layer, not here.
    pub alt: String,
    /// Optional width:height hint for layout before the blob loads.
    pub aspect_ratio: Option<AspectRatio>,
}

/// The subject of a reply arm: another post (by [`StrongRef`]) or a profile (by
/// [`Did`] — the "shout" case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplySubject {
    /// A reply to a specific post revision.
    Record(StrongRef),
    /// A shout on a User/Account profile, addressed by DID.
    Profile(Did),
}

/// A reply anchor: the thread `root` and the immediate `parent`. Its presence
/// on a [`FeedPost`] makes the post a comment/shout. v1 composes reply-to-root
/// only, so `parent == root`. (DD 30572573)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyRef {
    /// The root of the thread this reply belongs to.
    pub root: ReplySubject,
    /// The immediate subject being replied to.
    pub parent: ReplySubject,
}

/// A collaborator credit: the DID that contributed and the open-string capacity
/// it contributed in. A credit is a public, permanent, network-wide
/// cross-persona correlation surface — opt-out happens only at compose time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    /// The capacity the subject contributed in (open set, e.g. `artist`, `colors`).
    pub role: String,
    /// The credited collaborator's DID.
    pub did: Did,
}

/// The maturity self-labels a record carries — content-warning metadata that
/// travels with the content. An empty set means Safe; the field is always
/// present, so every post declares a posture. Correctness of the label is
/// enforced at the compose/publish layer, not here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelfLabels(pub Vec<String>);

impl SelfLabels {
    /// The Safe posture: no labels.
    pub fn safe() -> Self {
        Self(Vec::new())
    }

    /// Whether this is the Safe (empty) posture.
    pub fn is_safe(&self) -> bool {
        self.0.is_empty()
    }
}

/// The unified Zurfur content record: a gallery publication, a comment and a
/// profile shout are all this shape — a reply is a post with
/// [`reply`](FeedPost::reply) set. Mirrors `app.zurfur.feed.post`.
///
/// `created_at` and [`labels`](FeedPost::labels) are always present; everything
/// else is optional. The publish rules the lexicon cannot express are enforced
/// at the compose layer, not in this value type nor in the write adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedPost {
    /// Optional poster-authored free text (post body, comment, or shout).
    pub text: Option<String>,
    /// Optional single visual-media embed.
    pub embed: Option<Embed>,
    /// Optional reply anchor; its presence marks this post a comment/shout.
    pub reply: Option<ReplyRef>,
    /// Collaborator credits (may be empty).
    pub credits: Vec<Credit>,
    /// Required maturity self-labels (empty = Safe).
    pub labels: SelfLabels,
    /// Client-declared creation timestamp.
    pub created_at: DateTimeUtc,
}

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
mod tests {
    use super::*;

    #[test]
    fn at_uri_round_trips_through_string() {
        let uri = AtUri::new(
            Did::new("did:plc:abc123".to_string()),
            Nsid::new("app.zurfur.feed.post"),
            RecordKey::new("3laa7lepk2c"),
        );
        let s = uri.to_string();
        assert_eq!(s, "at://did:plc:abc123/app.zurfur.feed.post/3laa7lepk2c");
        assert_eq!(AtUri::parse(&s).unwrap(), uri);
    }

    #[test]
    fn at_uri_parse_rejects_malformed() {
        assert_eq!(
            AtUri::parse("did:plc:abc/app.zurfur.feed.post/rk"),
            Err(AtUriParseError::MissingScheme)
        );
        assert_eq!(
            AtUri::parse("at://did:plc:abc/app.zurfur.feed.post"),
            Err(AtUriParseError::Malformed)
        );
    }

    #[test]
    fn at_uri_parse_rejects_query_and_fragment() {
        assert_eq!(
            AtUri::parse("at://did:plc:abc/app.zurfur.feed.post/3laa7lepk2c?x=1"),
            Err(AtUriParseError::Malformed)
        );
        assert_eq!(
            AtUri::parse("at://did:plc:abc/app.zurfur.feed.post/3laa7lepk2c#frag"),
            Err(AtUriParseError::Malformed)
        );
    }

    #[test]
    fn feed_post_fixes_its_collection() {
        let record = PublicRecord::FeedPost(FeedPost {
            text: Some("hi".to_string()),
            embed: None,
            reply: None,
            credits: Vec::new(),
            labels: SelfLabels::safe(),
            created_at: chrono::Utc::now(),
        });
        assert_eq!(record.collection().as_str(), "app.zurfur.feed.post");
    }
}
