use crate::datetime::DateTimeUtc;
use crate::elements::did::Did;

use super::{BlobRef, StrongRef};

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
/// only, so `parent == root`.
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
