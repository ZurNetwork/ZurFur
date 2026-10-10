use super::*;

#[test]
fn levels_order_from_absent_to_open() {
    assert!(Level::Private < Level::Listed);
    assert!(Level::Listed < Level::Public);
    assert_eq!(Level::Private.max(Level::Public), Level::Public);
}
