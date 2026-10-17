use super::*;

#[test]
fn the_limit_is_ten_an_hour() {
    let one_hour = Duration::hours(1);

    assert_eq!(BUG_REPORT_LIMIT, 10);
    assert_eq!(BUG_REPORT_WINDOW, one_hour);
}
