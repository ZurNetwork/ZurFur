use super::*;

#[test]
fn trims_and_keeps_the_text() {
    let text = "  Fluffy  ".parse::<NonEmptyString>().expect("non-empty");
    assert_eq!(text.as_ref(), "Fluffy");
    assert_eq!(text.to_string(), "Fluffy");
}

#[test]
fn refuses_blank_input() {
    assert_eq!(
        "".parse::<NonEmptyString>(),
        Err(NonEmptyStringError::Empty)
    );
    assert_eq!(
        "   ".parse::<NonEmptyString>(),
        Err(NonEmptyStringError::Empty)
    );
    assert_eq!(
        NonEmptyString::try_from("\t\n".to_owned()),
        Err(NonEmptyStringError::Empty)
    );
}
