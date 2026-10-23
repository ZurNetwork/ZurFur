use super::*;

#[test]
fn zero_and_one_are_refused() {
    assert_eq!(NameOrdinal::try_from(0), Err(NameOrdinalError));
    assert_eq!(NameOrdinal::try_from(1), Err(NameOrdinalError));
}

#[test]
fn two_and_up_read_back_unchanged() {
    for number in [2, 7, u32::MAX] {
        let ordinal = NameOrdinal::try_from(number).expect("an ordinal");

        assert_eq!(u32::from(ordinal), number);
        assert_eq!(ordinal.to_string(), number.to_string());
    }
}
