use strum::VariantArray;

use super::*;

#[test]
fn every_soft_deleted_state_has_its_word() {
    let words: Vec<&'static str> = SoftDelete::VARIANTS
        .iter()
        .map(|state| state.into())
        .collect();

    assert_eq!(words, ["archived", "deactivated"]);
}
