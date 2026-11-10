use super::*;

#[test]
fn each_marker_names_its_seal_purpose() {
    assert_eq!(ForDidCustody::PURPOSE, SealPurpose::DidCustody);
    assert_eq!(ForSigninSecrets::PURPOSE, SealPurpose::SigninSecrets);
}
