use kernel::elements::maturity::MaturityRating;
use strum::VariantArray;

use super::*;
use crate::elements::commission::{LifecycleStep, test_commission};

/// The state of an Active commission holding `maturity`.
fn holding(maturity: Option<Maturity>) -> CommissionState {
    let mut commission = test_commission::at(LifecycleStep::Active);
    commission.maturity = maturity;
    CommissionState::from(&commission)
}

/// A rating of `rating`, graphic or not.
fn rated(rating: MaturityRating, graphic: bool) -> Maturity {
    Maturity { rating, graphic }
}

// A first rating and a change of either the rating or the Graphic flag are real sets, each
// carrying the rating held and the one given; the rating held, both halves alike, is a no-op.
#[test]
fn a_rating_replaces_the_held_one_and_the_held_one_is_a_no_op() {
    let unrated = holding(None);
    let safe = rated(MaturityRating::Safe, false);
    let held = holding(Some(safe));

    let first = rate_maturity(&unrated, safe).expect("a real set");
    assert_eq!((first.before(), first.after()), (None, safe));

    let suggestive = rated(MaturityRating::Suggestive, false);
    let re_rated = rate_maturity(&held, suggestive).expect("a real set");
    assert_eq!(
        (re_rated.before(), re_rated.after()),
        (Some(safe), suggestive)
    );

    let graphic = rated(MaturityRating::Safe, true);
    let flagged = rate_maturity(&held, graphic).expect("the Graphic flag alone is a change");
    assert_eq!((flagged.before(), flagged.after()), (Some(safe), graphic));

    assert_eq!(rate_maturity(&held, safe), None);
}

// No step refuses a rating: Draft, the terminal steps and Disputed all take one.
#[test]
fn no_step_refuses_a_rating() {
    let adult = rated(MaturityRating::Adult, true);
    for step in LifecycleStep::VARIANTS {
        let state = CommissionState::from(&test_commission::at(step.clone()));
        assert!(rate_maturity(&state, adult).is_some(), "{step:?}");
    }
}
