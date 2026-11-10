use strum::VariantArray;

use super::*;

#[test]
fn the_purpose_tokens_are_these() {
    let tokens: Vec<&'static str> = Purpose::VARIANTS
        .iter()
        .map(|purpose| <&'static str>::from(*purpose))
        .collect();

    assert_eq!(tokens, ["did_custody", "signin_secrets", "den_mac"]);
}

#[test]
fn the_seal_purpose_tokens_are_these() {
    let tokens: Vec<&'static str> = SealPurpose::VARIANTS
        .iter()
        .map(|purpose| <&'static str>::from(*purpose))
        .collect();

    assert_eq!(tokens, ["did_custody", "signin_secrets"]);
}

#[test]
fn the_mac_use_tokens_are_these() {
    let tokens: Vec<&'static str> = MacUse::VARIANTS
        .iter()
        .map(|mac_use| <&'static str>::from(*mac_use))
        .collect();

    assert_eq!(tokens, ["den_node", "den_changelog_entry"]);
}

#[test]
fn every_token_round_trips() {
    for purpose in Purpose::VARIANTS {
        let token = <&'static str>::from(*purpose);

        assert_eq!(purpose.to_string(), token);
        assert_eq!(token.parse::<Purpose>(), Ok(*purpose));
    }
    for purpose in SealPurpose::VARIANTS {
        let token = <&'static str>::from(*purpose);

        assert_eq!(purpose.to_string(), token);
        assert_eq!(token.parse::<SealPurpose>(), Ok(*purpose));
    }
    for mac_use in MacUse::VARIANTS {
        let token = <&'static str>::from(*mac_use);

        assert_eq!(mac_use.to_string(), token);
        assert_eq!(token.parse::<MacUse>(), Ok(*mac_use));
    }
}

#[test]
fn an_unknown_token_is_the_typed_refusal() {
    for bad in [
        "",
        "DidCustody",
        "DID_CUSTODY",
        "did-custody",
        "den_mac ",
        "root",
    ] {
        assert_eq!(bad.parse::<Purpose>(), Err(UnknownPurpose), "{bad:?}");
    }
    for bad in ["", "den_mac", "SigninSecrets", "signin"] {
        assert_eq!(
            bad.parse::<SealPurpose>(),
            Err(UnknownSealPurpose),
            "{bad:?}"
        );
    }
    for bad in ["", "den_mac", "DenNode", "den_changelog"] {
        assert_eq!(bad.parse::<MacUse>(), Err(UnknownMacUse), "{bad:?}");
    }
}

#[test]
fn a_seal_purpose_is_the_purpose_of_the_same_name() {
    let purposes: Vec<Purpose> = SealPurpose::VARIANTS
        .iter()
        .map(|seal_purpose| Purpose::from(*seal_purpose))
        .collect();

    assert_eq!(purposes, [Purpose::DidCustody, Purpose::SigninSecrets]);
}
