//! Public-boundary record value types — the domain's protocol-free vocabulary
//! for the records Zurfur publishes into an actor's atproto repo (ZMVP-105).
//!
//! These are the types the [`PublicRecords`](crate::ports::PublicRecords) port
//! speaks in. They mirror the merged `app.zurfur.feed.post` lexicon (DESIGN:
//! Lexicon `10354710`, Gallery Posts DD `29949954`) **as domain data**, carrying
//! *zero* AT-Protocol/`jacquard` types: the wire shape, CBOR, and CID computation
//! all live behind the boundary in `adapter-atproto`, so "nothing protocol-shaped
//! leaks past that crate" (DESIGN/"Domains and Applications").
//!
//! A record is addressed by an [`AtUri`] (`at://did/collection/rkey`) and, once
//! written, fingerprinted by a content-hash [`Cid`] — see [`RecordRef`] and
//! [`StrongRef`]. The one record kind v1 publishes is a [`FeedPost`]; [`PublicRecord`]
//! is the extensible envelope over it (one variant now, additively more later),
//! and the variant is what fixes the collection NSID.

use cid::Cid;

use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;

/// A collection NSID — the reverse-DNS name of the lexicon a record belongs to
/// (e.g. `app.zurfur.feed.post`).
///
/// A newtype for type-safety at the boundary, not a validating parser: the value
/// is held as the opaque string. The domain only ever originates the small,
/// fixed set of NSIDs it publishes (via [`PublicRecord::collection`]); a value
/// read back off the wire is validated by the adapter, not here.
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

/// A record key (`rkey`) — the per-collection identifier of a single record.
///
/// For `app.zurfur.feed.post` the key is a TID (a timestamp-ordered identifier)
/// minted by the repo on create; the domain treats it as an opaque string. A
/// newtype for type-safety, not a validating parser.
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
/// `at://<did>/<collection>/<rkey>`.
///
/// This is the domain-side [AT-URI](https://atproto.com/specs/at-uri-scheme)
/// restricted to the repo-record form the boundary needs (authority, collection,
/// and rkey; no fragment or query). The `put_record`, `delete_record`, and
/// `get_record` port methods each take one to address a record; a fresh
/// `create_record` returns one (inside a [`RecordRef`]) rather than taking it.
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

    /// Parse an `at://<did>/<collection>/<rkey>` string.
    ///
    /// The authority (a DID) contains colons but no slashes, and neither the
    /// collection NSID nor the rkey contains a slash, so the three path segments
    /// after `at://` split unambiguously on `/`. A query (`?…`) or fragment
    /// (`#…`) — legal in the full AT-URI grammar — is rejected: the repo-record
    /// form has neither, and none of the three parts may contain those
    /// characters.
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
/// [`Cid`] of the exact revision pointed at.
///
/// Mirrors `com.atproto.repo.strongRef` — a pointer that is invalidated if the
/// target record changes, because the CID would no longer match. Used inside a
/// [`ReplySubject::Record`] arm to anchor a reply to a specific post revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongRef {
    /// The referenced record's address.
    pub uri: AtUri,
    /// The content hash of the referenced revision.
    pub cid: Cid,
}

/// The address + content hash a write returns: where the record landed
/// ([`AtUri`]) and the [`Cid`] of the revision just written.
///
/// Returned by [`create`](crate::ports::PublicRecords::create_record) and
/// [`put`](crate::ports::PublicRecords::put_record). Structurally identical to a
/// [`StrongRef`]; kept a distinct type because it names a *write result* (what the
/// repo now holds) rather than a *reference to* another record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRef {
    /// Where the record was written.
    pub uri: AtUri,
    /// The content hash of the written revision.
    pub cid: Cid,
}

/// A reference to an uploaded blob: its content-address [`Cid`] plus the
/// mime type and byte size the repo recorded.
///
/// Returned by [`upload_blob`](crate::ports::PublicRecords::upload_blob) and
/// embedded in a record via [`Embed::blob`]. The [`Cid`] is the same
/// content-address a [`BlobId`](crate::elements::blob::BlobId) wraps — see
/// [`BlobRef::id`] — so byte-identical blobs share a ref network-wide.
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
    /// The blob's content-addressed identity — the same
    /// [`BlobId`](crate::elements::blob::BlobId) the rest of the domain refers to
    /// a blob by. A [`BlobRef`] is that id plus the mime/size the embed needs.
    pub fn id(&self) -> crate::elements::blob::BlobId {
        crate::elements::blob::BlobId::new(self.cid)
    }
}

/// A width:height aspect ratio hint for a media embed (both ≥ 1).
///
/// Optional layout metadata so a client can reserve space before the blob loads;
/// may be approximate. Mirrors `app.zurfur.embed.media#aspectRatio`.
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
    /// Required alt-text description of the media (accessibility-first). The
    /// non-blank rule is enforced at the compose/publish layer, not here (ZMVP-108).
    pub alt: String,
    /// Optional width:height hint for layout before the blob loads.
    pub aspect_ratio: Option<AspectRatio>,
}

/// The subject of a reply arm: either another post (by [`StrongRef`]) or a
/// profile (by [`Did`] — the "shout" case, replying to a User/Account identity).
///
/// Mirrors the `app.zurfur.feed.post#replyRef` union of a
/// `com.atproto.repo.strongRef` and a bare-DID `#didSubject`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplySubject {
    /// A reply to a specific post revision.
    Record(StrongRef),
    /// A shout on a User/Account profile, addressed by DID.
    Profile(Did),
}

/// A reply anchor: the thread `root` and the immediate `parent`.
///
/// Its presence on a [`FeedPost`] is what makes the post a comment/shout rather
/// than a gallery publication. v1 composes reply-to-root only (so `parent ==
/// root`); the distinct-parent shape is reserved for later threaded rendering
/// (Replyable DD `30572573`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyRef {
    /// The root of the thread this reply belongs to.
    pub root: ReplySubject,
    /// The immediate subject being replied to.
    pub parent: ReplySubject,
}

/// A collaborator credit: the DID that contributed and the capacity (`role`) it
/// contributed in. Mirrors `app.zurfur.feed.defs#credit`.
///
/// `role` is an open string (unknown roles render verbatim). A credit is a
/// **public, permanent, network-wide cross-persona correlation surface** — opt-out
/// happens at compose time, never by mutating a published record (Gallery Posts
/// DD `29949954`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    /// The capacity the subject contributed in (open set, e.g. `artist`, `colors`).
    pub role: String,
    /// The credited collaborator's DID.
    pub did: Did,
}

/// The maturity self-labels a record carries — content-warning metadata that
/// travels with the content to any appview.
///
/// An **empty** set means Safe (the protocol norm: safety metadata rides with the
/// content; Zurfur additionally *requires* the field so every post declares a
/// posture explicitly). Mirrors `com.atproto.label.defs#selfLabels`. The
/// "a mature work must carry the correct label" rule is enforced at the
/// compose/publish layer, not here (ZMVP-108).
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

/// The unified Zurfur content record: a gallery publication, a comment, and a
/// profile shout are all this one shape — a reply is simply a post with
/// [`reply`](FeedPost::reply) set. Mirrors the `app.zurfur.feed.post` lexicon.
///
/// `created_at` and [`labels`](FeedPost::labels) are always present (Safe = an
/// empty label set); everything else is optional. The app-side publish rules the
/// lexicon cannot express (≥1 of text/embed, conditional maturity label, image
/// sub-cap, non-blank alt) are enforced at the compose/publish layer, **not** in
/// this value type nor in the write adapter (ZMVP-108) — ZMVP-105 writes
/// faithfully and relies on the repo to validate structure.
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

/// The extensible envelope over the public records Zurfur can publish.
///
/// One variant today ([`FeedPost`]); more may be **added** additively as new
/// record kinds are introduced (a one-way door, like the lexicons themselves).
/// The variant fixes the collection NSID — see [`collection`](PublicRecord::collection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicRecord {
    /// An `app.zurfur.feed.post` record.
    FeedPost(FeedPost),
}

impl PublicRecord {
    /// The collection NSID this record belongs to — fixed by the variant, so a
    /// caller never has to (and never gets to) pick a collection that disagrees
    /// with the record body.
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
