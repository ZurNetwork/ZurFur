use domain::elements::handle::Handle;
use proptest::prelude::*;

use super::*;

/// Handles of up to five labels, parsed from mixed-case text.
fn handle() -> impl Strategy<Value = Handle> {
    "[a-zA-Z0-9]([a-zA-Z0-9-]{0,20}[a-zA-Z0-9])?(\\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,20}[a-zA-Z0-9])?){1,4}"
        .prop_filter_map("a valid handle", |text| text.parse::<Handle>().ok())
}

proptest! {
    /// Every handle passes the Den's name rule, unchanged, so an Account whose stored name
    /// fails the rule always shows its handle.
    #[test]
    fn every_handle_is_a_name_of_the_same_text(handle in handle()) {
        let name = NodeName::try_from(handle.as_ref()).ok();

        prop_assert_eq!(name.as_ref().map(AsRef::as_ref), Some(handle.as_ref()));
    }
}
