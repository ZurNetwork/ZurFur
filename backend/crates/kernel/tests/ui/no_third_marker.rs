//! No crate outside kernel can add a seal purpose marker.

use kernel::elements::custody::{SealPurpose, SealPurposeMarker};

enum ForSomethingElse {}

impl SealPurposeMarker for ForSomethingElse {
    const PURPOSE: SealPurpose = SealPurpose::DidCustody;
}

fn main() {}
