use proptest::prelude::*;

use super::*;

/// Text over the segment alphabet, up to the segment cap.
fn segment_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9._:%~-]{1,2048}"
}

proptest! {
    /// A DID that is a valid segment, as every root's key is, is always a valid name, unchanged:
    /// the root's last fallback never fails.
    #[test]
    fn every_segment_is_a_name_of_the_same_text(text in segment_text()) {
        prop_assume!(Segment::try_from(text.as_str()).is_ok());

        let name = NodeName::try_from(text.as_str()).ok();

        prop_assert_eq!(name.as_ref().map(AsRef::as_ref), Some(text.as_str()));
    }

    /// A `did:plc` DID is always a valid name, unchanged.
    #[test]
    fn every_plc_did_is_a_name_of_the_same_text(id in "[a-z2-7]{24}") {
        let did = format!("did:plc:{id}");

        let name = NodeName::try_from(did.as_str()).ok();

        prop_assert_eq!(name.as_ref().map(AsRef::as_ref), Some(did.as_str()));
    }
}
