use super::*;

// The axis table: Safe emits none, Suggestive -> sexual, Nudity -> nudity,
// Adult -> porn; Graphic rides alongside any rating.
#[test]
fn self_labels_follow_the_dd_axis_table() {
    assert_eq!(MaturityRating::Safe.self_label(), None);
    assert_eq!(MaturityRating::Suggestive.self_label(), Some("sexual"));
    assert_eq!(MaturityRating::Nudity.self_label(), Some("nudity"));
    assert_eq!(MaturityRating::Adult.self_label(), Some("porn"));

    let safe = Maturity {
        rating: MaturityRating::Safe,
        graphic: false,
    };
    assert!(safe.self_labels().is_empty(), "Safe + not graphic = empty");

    let graphic_safe = Maturity {
        rating: MaturityRating::Safe,
        graphic: true,
    };
    assert_eq!(
        graphic_safe.self_labels(),
        vec!["graphic-media"],
        "Graphic is orthogonal — it rides even on Safe",
    );

    let graphic_adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: true,
    };
    assert_eq!(graphic_adult.self_labels(), vec!["porn", "graphic-media"]);
}
