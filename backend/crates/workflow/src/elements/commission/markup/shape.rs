use serde::{Deserialize, Serialize};

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
