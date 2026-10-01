use super::super::CompositionLabelError;
use super::*;

// The composition labels share one validation contract.
#[test]
fn composition_labels_share_one_validation_contract() {
    assert_eq!(" main ".parse::<TabName>().unwrap().as_ref(), "main");
    assert_eq!(
        " content ".parse::<SurfaceName>().unwrap().as_ref(),
        "content"
    );
    assert_eq!(" note ".parse::<ElementType>().unwrap().as_ref(), "note");
    assert_eq!(" body ".parse::<Band>().unwrap().as_ref(), "body");

    assert_eq!(
        "  ".parse::<SurfaceName>(),
        Err(CompositionLabelError::Empty)
    );
    assert_eq!(
        "a\nb".parse::<ElementType>(),
        Err(CompositionLabelError::ControlCharacter)
    );
    assert_eq!(
        "x".repeat(LABEL_MAX_CHARS + 1).parse::<TabName>(),
        Err(CompositionLabelError::TooLong)
    );
    assert!("x".repeat(LABEL_MAX_CHARS).parse::<Band>().is_ok());
}

// The satellite type tags live in one place.
#[test]
fn the_satellite_type_tags_are_stable() {
    assert_eq!(ElementType::slot().as_ref(), ElementType::SLOT_TAG);
    assert_eq!(ElementType::seat().as_ref(), ElementType::SEAT_TAG);
    assert_eq!(ElementType::SLOT_TAG, "slot");
    assert_eq!(ElementType::SEAT_TAG, "seat");
}
