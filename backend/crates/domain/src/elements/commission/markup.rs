//! Commission **Markup** (ZMVP-90): coordinate-anchored annotation a Participant
//! attaches to a file entry in the review loop (DESIGN/Commission — "File entries
//! and Markup"; Engineer ruling E14 2026-07-05).
//!
//! Stored RAW and parsed by the frontend: core validates, stores, and serves the
//! data; the drawing canvas UI is a future first-party Plugin. A markup rides the
//! `markup_added` changelog entry's jsonb payload (no table of its own),
//! referencing the [`FileKey`](super::file::FileKey) of a validated existing file
//! entry — and is served back **untransformed**. Untransformed means *semantic*
//! fidelity: no coordinate transformation, ever — but the payload lives in a
//! Postgres `jsonb` column, which normalizes object key order (and the typed
//! round-trip renders every number as a float), so byte-for-byte identity of the
//! JSON text is not promised.
//!
//! **Validation is strict, on the way in, and it is the only gate there will ever
//! be**: the changelog is append-only (ZMVP-87 AC4), so malformed markup accepted
//! today would be malformed forever. Unknown shapes and unknown fields are
//! rejected by shape ([`deny_unknown_fields`]); [`Markup::validate`] then enforces
//! the numeric and text rules serde cannot express.
//!
//! **Coordinates are normalized 0–1 floats** relative to the annotated image
//! (ruling E14): markup survives client-side scaling and thumbnailing, and the
//! server never needs the image's pixel dimensions (the blob is opaque to it).
//! Containment is deliberately NOT enforced — a circle at the image's edge may
//! overflow it (`cx + r > 1`); renderers clip. Only each stored value is bounded.
//!
//! Threading, persistence on file replacement, the annotate-matrix, and retention
//! are explicitly deferred to the File Activity & Markup 1DD; markup immutability
//! (no edit, no delete) is already settled by the changelog's append-only shape.
//!
//! [`deny_unknown_fields`]: https://serde.rs/container-attrs.html#deny_unknown_fields

use serde::{Deserialize, Serialize};

/// One Markup: a [`shape`](Self::shape) anchored in normalized 0–1 image space,
/// with an optional [`text`](Self::text) comment — exactly the wire body of
/// `POST /commissions/{id}/files/{file_id}/markup`, and exactly what the
/// `markup_added` entry's payload carries back out (ruling E14).
///
/// Deserialization is strict (`deny_unknown_fields`): nothing rides along with a
/// markup — in particular nothing status-shaped (the always-explicit rule). After
/// deserializing, call [`validate`](Self::validate); serde alone cannot bound the
/// numbers or cap the text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markup {
    /// The annotation's geometry, in normalized 0–1 image coordinates.
    pub shape: MarkupShape,
    /// The optional comment anchored at the shape — part of the markup datum
    /// itself (each shape "may carry an optional text comment",
    /// DESIGN/Commission), distinct from a changelog entry's free-text note.
    /// Absent stays absent on the way back out. Capped at
    /// [`MAX_TEXT_CHARS`](Self::MAX_TEXT_CHARS) characters and must not be blank
    /// when present; stored as submitted (untransformed — not even trimmed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The closed vocabulary of markup geometry (ruling E14): circle, rectangle, or
/// freehand stroke, every number a normalized 0–1 float. Externally tagged on the
/// wire — `{"circle": {"cx": …, "cy": …, "r": …}}` — with unknown variants *and*
/// unknown fields inside a variant rejected (`deny_unknown_fields`): the strict
/// write gate of an append-only record.
///
/// Positions (`cx`/`cy`/`x`/`y`, freehand points) live in `[0, 1]`; extents
/// (`r`/`w`/`h`) in `(0, 1]` — a zero extent is an invisible, degenerate shape.
/// JSON cannot carry `NaN`/`Infinity`, so every deserialized value is finite by
/// construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MarkupShape {
    /// A circle: center (`cx`, `cy`) and radius `r`.
    Circle {
        /// The center's horizontal position, `0..=1`.
        cx: f64,
        /// The center's vertical position, `0..=1`.
        cy: f64,
        /// The radius, `0 < r <= 1` (in the normalized space; may overflow the
        /// image at the edges — renderers clip).
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
    /// A freehand stroke: an ordered polyline of `[x, y]` points. At least
    /// [`MIN_FREEHAND_POINTS`](Markup::MIN_FREEHAND_POINTS) (one point is a dot,
    /// not a stroke — use a circle), at most
    /// [`MAX_FREEHAND_POINTS`](Markup::MAX_FREEHAND_POINTS) (the record is
    /// append-only; an unbounded stroke would bloat it forever). A point is
    /// exactly two numbers — serde rejects `[x]`, `[x, y, z]`, and non-numbers
    /// by shape.
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
    /// An extent (`r`/`w`/`h`) is not in `(0, 1]` — zero/negative is a
    /// degenerate, invisible shape; over 1 exceeds the whole image.
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
    /// The text comment's length cap, in characters — an annotation is a remark
    /// anchored at a shape, not a document.
    pub const MAX_TEXT_CHARS: usize = 2000;

    /// The fewest points a freehand stroke may carry: two — a stroke is a line;
    /// a single point is a dot (use a circle).
    pub const MIN_FREEHAND_POINTS: usize = 2;

    /// The most points a freehand stroke may carry. Generous for a hand-drawn
    /// stroke (a minute of 60 Hz sampling), tight enough that one markup can't
    /// bloat the append-only record forever.
    pub const MAX_FREEHAND_POINTS: usize = 4096;

    /// The strict write gate (ruling E14): enforce everything serde's shape
    /// checking cannot — positions in `[0, 1]`, extents in `(0, 1]`, freehand
    /// point-count bounds, and the text rules (non-blank when present, at most
    /// [`MAX_TEXT_CHARS`](Self::MAX_TEXT_CHARS) characters). The first violation
    /// is returned; a markup that passes is stored — and served — exactly as
    /// submitted.
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

/// An extent (radius/width/height): `(0, 1]` — visible, and no larger than the
/// whole image.
fn extent(field: &'static str, value: f64) -> Result<(), MarkupError> {
    if value > 0.0 && value <= 1.0 {
        Ok(())
    } else {
        Err(MarkupError::ExtentOutOfRange(field, value))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(value: serde_json::Value) -> Result<Markup, serde_json::Error> {
        serde_json::from_value(value)
    }

    // The wire shape: externally tagged snake_case shapes, optional text.
    #[test]
    fn markup_deserializes_from_its_wire_shape() {
        let markup = parse(json!({
            "shape": { "circle": { "cx": 0.5, "cy": 0.25, "r": 0.125 } },
            "text": "here",
        }))
        .expect("a well-formed circle parses");
        assert_eq!(
            markup.shape,
            MarkupShape::Circle {
                cx: 0.5,
                cy: 0.25,
                r: 0.125
            }
        );
        assert_eq!(markup.text.as_deref(), Some("here"));

        let markup = parse(json!({
            "shape": { "freehand": { "points": [[0.1, 0.2], [0.3, 0.4]] } },
        }))
        .expect("a well-formed freehand parses");
        assert_eq!(markup.text, None);
    }

    // Strict by shape: unknown variants, unknown fields (top-level and inside a
    // variant), and malformed points are refused at deserialization.
    #[test]
    fn unknown_shapes_and_fields_do_not_deserialize() {
        for bad in [
            json!({ "shape": { "arrow": { "cx": 0.5, "cy": 0.5, "r": 0.1 } } }),
            json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1, "color": "red" } } }),
            json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5 } } }),
            json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.5, "r": 0.1 } }, "status": "x" }),
            json!({ "shape": { "freehand": { "points": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]] } } }),
            json!({ "shape": { "freehand": { "points": [[0.1], [0.2]] } } }),
            json!({ "shape": { "circle": { "cx": "0.5", "cy": 0.5, "r": 0.1 } } }),
            json!({ "text": "no shape at all" }),
        ] {
            assert!(parse(bad.clone()).is_err(), "must be rejected: {bad}");
        }
    }

    // The numeric gate: positions in [0, 1], extents in (0, 1].
    #[test]
    fn coordinates_are_bounded_to_the_normalized_space() {
        let circle = |cx, cy, r| Markup {
            shape: MarkupShape::Circle { cx, cy, r },
            text: None,
        };
        assert!(
            circle(0.0, 1.0, 1.0).validate().is_ok(),
            "the bounds are in"
        );
        assert_eq!(
            circle(-0.1, 0.5, 0.1).validate(),
            Err(MarkupError::CoordinateOutOfRange("cx", -0.1))
        );
        assert_eq!(
            circle(0.5, 1.5, 0.1).validate(),
            Err(MarkupError::CoordinateOutOfRange("cy", 1.5))
        );
        assert_eq!(
            circle(0.5, 0.5, 0.0).validate(),
            Err(MarkupError::ExtentOutOfRange("r", 0.0)),
            "a zero radius is a degenerate shape"
        );
        assert_eq!(
            circle(0.5, 0.5, 1.5).validate(),
            Err(MarkupError::ExtentOutOfRange("r", 1.5))
        );

        let rectangle = Markup {
            shape: MarkupShape::Rectangle {
                x: 0.9,
                y: 0.9,
                w: 0.2,
                h: 0.2,
            },
            text: None,
        };
        assert!(
            rectangle.validate().is_ok(),
            "containment is not enforced — edge shapes may overflow; renderers clip"
        );
        assert_eq!(
            Markup {
                shape: MarkupShape::Rectangle {
                    x: 0.1,
                    y: 0.1,
                    w: 0.0,
                    h: 0.5,
                },
                text: None,
            }
            .validate(),
            Err(MarkupError::ExtentOutOfRange("w", 0.0))
        );
    }

    // The freehand gate: 2..=MAX points, every coordinate bounded.
    #[test]
    fn freehand_strokes_are_bounded() {
        let freehand = |points: Vec<[f64; 2]>| Markup {
            shape: MarkupShape::Freehand { points },
            text: None,
        };
        assert_eq!(freehand(vec![]).validate(), Err(MarkupError::TooFewPoints));
        assert_eq!(
            freehand(vec![[0.5, 0.5]]).validate(),
            Err(MarkupError::TooFewPoints),
            "one point is a dot, not a stroke"
        );
        assert!(freehand(vec![[0.0, 0.0], [1.0, 1.0]]).validate().is_ok());
        assert_eq!(
            freehand(vec![[0.1, 0.2], [0.3, 1.5]]).validate(),
            Err(MarkupError::CoordinateOutOfRange("points[].y", 1.5))
        );
        assert_eq!(
            freehand(vec![[0.5, 0.5]; Markup::MAX_FREEHAND_POINTS + 1]).validate(),
            Err(MarkupError::TooManyPoints)
        );
        assert!(
            freehand(vec![[0.5, 0.5]; Markup::MAX_FREEHAND_POINTS])
                .validate()
                .is_ok(),
            "exactly at the cap is fine"
        );
    }

    // The text gate: absent is fine, blank is not, the cap is characters.
    #[test]
    fn text_must_be_meaningful_when_present() {
        let with_text = |text: Option<String>| Markup {
            shape: MarkupShape::Circle {
                cx: 0.5,
                cy: 0.5,
                r: 0.1,
            },
            text,
        };
        assert!(with_text(None).validate().is_ok());
        assert!(with_text(Some("fluffier!".into())).validate().is_ok());
        assert_eq!(
            with_text(Some("   ".into())).validate(),
            Err(MarkupError::TextBlank)
        );
        assert_eq!(
            with_text(Some("x".repeat(Markup::MAX_TEXT_CHARS + 1))).validate(),
            Err(MarkupError::TextTooLong)
        );
        assert!(
            with_text(Some("é".repeat(Markup::MAX_TEXT_CHARS)))
                .validate()
                .is_ok(),
            "the cap counts characters, not bytes"
        );
    }

    // Untransformed round-trip: a validated markup re-serializes to exactly the
    // JSON it was parsed from — text stays absent when absent, nothing is
    // renamed, reordered semantically, or rescaled.
    #[test]
    fn a_markup_round_trips_to_the_same_json() {
        for value in [
            json!({ "shape": { "circle": { "cx": 0.5, "cy": 0.25, "r": 0.125 } }, "text": "t" }),
            json!({ "shape": { "rectangle": { "x": 0.25, "y": 0.25, "w": 0.5, "h": 0.375 } } }),
            json!({ "shape": { "freehand": { "points": [[0.125, 0.5], [0.25, 0.625]] } } }),
        ] {
            let markup = parse(value.clone()).expect("parses");
            markup.validate().expect("valid");
            assert_eq!(
                serde_json::to_value(&markup).expect("serializes"),
                value,
                "the stored form is the submitted form"
            );
        }
    }
}
