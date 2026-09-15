use serde::{Deserialize, Serialize};

use super::{MarkupError, MarkupShape};

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

#[cfg(test)]
mod tests;
