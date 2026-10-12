use strum::VariantArray;

use super::*;

#[test]
fn every_kind_has_its_word() {
    let words: Vec<&'static str> = NodeKind::VARIANTS.iter().map(|kind| kind.into()).collect();

    assert_eq!(words, ["directory", "file", "symlink"]);
}
