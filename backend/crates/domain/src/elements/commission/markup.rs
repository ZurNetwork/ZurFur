//! Commission Markup: coordinate-anchored annotation a Participant attaches to
//! a file entry in the review loop.
//!
//! Stored raw and parsed by the frontend. Coordinates are normalized 0–1 floats
//! relative to the image; containment is NOT enforced — renderers clip.
//! Validation on the way in is the only gate there will be, because the record
//! is append-only: unknown shapes and fields are refused by serde, and
//! [`Markup::validate`] enforces what serde cannot.

use std::ops::Deref;

use serde::{Deserialize, Serialize};

use super::{CommissionId, file::FileKey};
use crate::{datetime::DateTimeUtc, elements::user::UserId};

/// One Markup: a [`shape`](Self::shape) anchored in normalized 0–1 image space,
/// with an optional [`text`](Self::text) comment. Deserialization is strict;
/// call [`validate`](Self::validate) afterwards — serde alone cannot bound the
/// numbers or cap the text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markup {
    /// The annotation's geometry, in normalized 0–1 image coordinates.
    pub shape: MarkupShape,
    /// The optional comment anchored at the shape, distinct from a changelog
    /// entry's note. Non-blank when present, at most
    /// [`MAX_TEXT_CHARS`](Self::MAX_TEXT_CHARS); stored untransformed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The closed vocabulary of markup geometry: circle, rectangle, or freehand
/// stroke. Externally tagged on the wire — `{"circle": {…}}` — with unknown
/// variants and unknown fields both rejected.
///
/// Positions live in `[0, 1]`, extents in `(0, 1]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MarkupShape {
    /// A circle: center (`cx`, `cy`) and radius `r`.
    Circle {
        /// The center's horizontal position, `0..=1`.
        cx: f64,
        /// The center's vertical position, `0..=1`.
        cy: f64,
        /// The radius, `0 < r <= 1`; may overflow the image at the edges.
        r: f64,
    },
    /// An axis-aligned rectangle: top-left corner (`x`, `y`) and size (`w`, `h`).
    Rectangle {
        /// The top-left corner's horizontal position, `0..=1`.
        x: f64,
        /// The top-left corner's vertical position, `0..=1`.
        y: f64,
        /// The width, `0 < w <= 1`.
        w: f64,
        /// The height, `0 < h <= 1`.
        h: f64,
    },
    /// A freehand stroke: an ordered polyline of `[x, y]` points, between
    /// [`MIN_FREEHAND_POINTS`](Markup::MIN_FREEHAND_POINTS) and
    /// [`MAX_FREEHAND_POINTS`](Markup::MAX_FREEHAND_POINTS) of them.
    Freehand {
        /// The stroke's points, each `[x, y]` with both in `0..=1`.
        points: Vec<[f64; 2]>,
    },
}

/// Why a structurally well-formed [`Markup`] was rejected by
/// [`Markup::validate`] — the rules serde's shape checking cannot express.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkupError {
    /// A position coordinate lies outside the normalized `0..=1` space. Carries
    /// the field name and the offending value.
    CoordinateOutOfRange(&'static str, f64),
    /// An extent (`r`/`w`/`h`) is not in `(0, 1]`.
    ExtentOutOfRange(&'static str, f64),
    /// A freehand stroke with fewer than
    /// [`MIN_FREEHAND_POINTS`](Markup::MIN_FREEHAND_POINTS) points.
    TooFewPoints,
    /// A freehand stroke with more than
    /// [`MAX_FREEHAND_POINTS`](Markup::MAX_FREEHAND_POINTS) points.
    TooManyPoints,
    /// A text comment that is present but blank (empty or whitespace-only).
    TextBlank,
    /// A text comment longer than [`MAX_TEXT_CHARS`](Markup::MAX_TEXT_CHARS)
    /// characters.
    TextTooLong,
}

impl std::fmt::Display for MarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkupError::CoordinateOutOfRange(field, value) => {
                write!(f, "coordinate {field} = {value} must be within 0..=1")
            }
            MarkupError::ExtentOutOfRange(field, value) => {
                write!(f, "extent {field} = {value} must be > 0 and <= 1")
            }
            MarkupError::TooFewPoints => write!(
                f,
                "a freehand stroke needs at least {} points",
                Markup::MIN_FREEHAND_POINTS
            ),
            MarkupError::TooManyPoints => write!(
                f,
                "a freehand stroke may carry at most {} points",
                Markup::MAX_FREEHAND_POINTS
            ),
            MarkupError::TextBlank => write!(f, "markup text must not be blank when present"),
            MarkupError::TextTooLong => write!(
                f,
                "markup text must be at most {} characters",
                Markup::MAX_TEXT_CHARS
            ),
        }
    }
}

impl std::error::Error for MarkupError {}

impl Markup {
    /// The text comment's length cap, in characters.
    pub const MAX_TEXT_CHARS: usize = 2000;

    /// The fewest points a freehand stroke may carry: two.
    pub const MIN_FREEHAND_POINTS: usize = 2;

    /// The most points a freehand stroke may carry.
    pub const MAX_FREEHAND_POINTS: usize = 4096;

    /// Enforce what serde cannot: positions in `[0, 1]`, extents in `(0, 1]`,
    /// the freehand point-count bounds, and the text rules. Returns the first
    /// violation; a markup that passes is stored exactly as submitted.
    ///
    /// ```
    /// use domain::elements::commission::{Markup, MarkupError, MarkupShape};
    ///
    /// let ok = Markup {
    ///     shape: MarkupShape::Circle { cx: 0.5, cy: 0.5, r: 0.1 },
    ///     text: Some("fluffier".to_string()),
    /// };
    /// assert!(ok.validate().is_ok());
    ///
    /// let out = Markup {
    ///     shape: MarkupShape::Circle { cx: 1.5, cy: 0.5, r: 0.1 },
    ///     text: None,
    /// };
    /// assert_eq!(out.validate(), Err(MarkupError::CoordinateOutOfRange("cx", 1.5)));
    /// ```
    pub fn validate(&self) -> Result<(), MarkupError> {
        match &self.shape {
            MarkupShape::Circle { cx, cy, r } => {
                coordinate("cx", *cx)?;
                coordinate("cy", *cy)?;
                extent("r", *r)?;
            }
            MarkupShape::Rectangle { x, y, w, h } => {
                coordinate("x", *x)?;
                coordinate("y", *y)?;
                extent("w", *w)?;
                extent("h", *h)?;
            }
            MarkupShape::Freehand { points } => {
                if points.len() < Self::MIN_FREEHAND_POINTS {
                    return Err(MarkupError::TooFewPoints);
                }
                if points.len() > Self::MAX_FREEHAND_POINTS {
                    return Err(MarkupError::TooManyPoints);
                }
                for [x, y] in points {
                    coordinate("points[].x", *x)?;
                    coordinate("points[].y", *y)?;
                }
            }
        }
        if let Some(text) = &self.text {
            if text.trim().is_empty() {
                return Err(MarkupError::TextBlank);
            }
            if text.chars().count() > Self::MAX_TEXT_CHARS {
                return Err(MarkupError::TextTooLong);
            }
        }
        Ok(())
    }
}

/// A position in the normalized image space: `0..=1`.
fn coordinate(field: &'static str, value: f64) -> Result<(), MarkupError> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(MarkupError::CoordinateOutOfRange(field, value))
    }
}

/// An extent (radius/width/height): `(0, 1]`.
fn extent(field: &'static str, value: f64) -> Result<(), MarkupError> {
    if value > 0.0 && value <= 1.0 {
        Ok(())
    } else {
        Err(MarkupError::ExtentOutOfRange(field, value))
    }
}

/// The app-private, opaque key of one stored markup (UUIDv7) — the
/// `commission_markup` row's primary key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkupKey(uuid::Uuid);

impl MarkupKey {
    /// Wrap an already-minted UUIDv7.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh key for a new markup. Sorts as creation order, so a
    /// per-file read needs no separate ordering column.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for MarkupKey {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// One stored markup: a validated [`Markup`] anchored to a file entry, with the
/// Participant who drew it and when. Canonical for the geometry; written on the
/// same [`UnitOfWork`](crate::ports::UnitOfWork) as its `markup_added` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionMarkup {
    /// The markup's opaque key and this row's primary key.
    pub id: MarkupKey,
    /// The commission whose review loop the markup belongs to; every read
    /// scopes by it, so a key from another commission stays invisible.
    pub commission_id: CommissionId,
    /// The annotated file entry, enforced as a pair with the commission.
    pub file_id: FileKey,
    /// The Participant who drew it. Carries no foreign key onto the actor
    /// tables, so shared history survives a tombstone.
    pub added_by: UserId,
    /// The annotation itself, already past [`Markup::validate`]. Stored and
    /// served untransformed.
    pub markup: Markup,
    /// When the markup was drawn.
    pub created_at: DateTimeUtc,
}

#[cfg(test)]
mod tests;
