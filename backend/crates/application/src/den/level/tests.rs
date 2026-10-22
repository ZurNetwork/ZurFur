use strum::VariantArray;

use super::*;

// Each level is Atta's level of the same name.
#[test]
fn every_level_maps_to_atta_s_same_level() {
    let mapped: Vec<atta::Level> = Level::VARIANTS.iter().copied().map(atta_level).collect();

    assert_eq!(
        mapped,
        [
            atta::Level::Private,
            atta::Level::Listed,
            atta::Level::Public,
        ]
    );
}

// The map keeps the order: a lower level is a lower level in Atta too.
#[test]
fn atta_level_keeps_the_order() {
    for low in Level::VARIANTS {
        for high in Level::VARIANTS {
            assert_eq!(
                low.cmp(high),
                atta_level(*low).cmp(&atta_level(*high)),
                "{low} against {high}",
            );
        }
    }
}

// Atta's answer maps back to the level it came from.
#[test]
fn den_level_undoes_atta_level() {
    for level in Level::VARIANTS {
        assert_eq!(den_level(atta_level(*level)), *level);
    }
}
