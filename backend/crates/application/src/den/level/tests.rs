use strum::VariantArray;

use super::*;

#[test]
fn every_level_has_its_word() {
    let words: Vec<&'static str> = NodeLevel::VARIANTS
        .iter()
        .map(|level| level.into())
        .collect();

    assert_eq!(words, ["private", "listed", "public"]);
}
