use strum::VariantArray;

use super::*;

// The refusal tokens, pinned literally and in declaration order.
#[test]
fn refusal_tokens_are_pinned_literally() {
    let tokens: Vec<&'static str> = CommissionRefusal::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();

    assert_eq!(
        tokens,
        [
            "lifecycle_move_refused",
            "commission_disputed",
            "no_deadline",
            "commission_late",
            "nothing_owed"
        ]
    );
}
