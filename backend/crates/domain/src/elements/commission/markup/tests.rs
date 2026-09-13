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

// Unknown variants, unknown fields, and malformed points are refused at
// deserialization.
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

// A validated markup re-serializes to exactly the JSON it was parsed from.
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
