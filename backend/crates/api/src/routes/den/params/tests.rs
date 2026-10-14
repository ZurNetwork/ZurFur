use strum::VariantArray;

use super::*;

/// A well-formed token at offset 1000.
const TOKEN: &str = "0100000000000003e8";

fn token() -> PageToken {
    TOKEN.parse::<PageToken>().expect("a well-formed token")
}

#[test]
fn the_known_parameters_are_spelled_as_their_json_names() {
    let tokens: Vec<&'static str> = Parameter::VARIANTS
        .iter()
        .map(|parameter| <&'static str>::from(*parameter))
        .collect();

    assert_eq!(tokens, ["includeDeleted", "pageToken"]);
}

#[test]
fn an_empty_query_is_the_default() {
    assert_eq!("".parse::<DenParams>(), Ok(DenParams::default()));
    assert_eq!("&&".parse::<DenParams>(), Ok(DenParams::default()));
}

#[test]
fn both_parameters_are_read() {
    let expected = DenParams {
        include_deleted: true,
        page: Some(token()),
    };

    let parsed = format!("includeDeleted=true&pageToken={TOKEN}").parse::<DenParams>();

    assert_eq!(parsed, Ok(expected));
}

#[test]
fn include_deleted_false_is_read() {
    let expected = DenParams {
        include_deleted: false,
        page: None,
    };

    assert_eq!("includeDeleted=false".parse::<DenParams>(), Ok(expected));
}

#[test]
fn keys_and_values_are_decoded_once() {
    let expected = DenParams {
        include_deleted: true,
        page: None,
    };

    assert_eq!("include%44eleted=%74rue".parse::<DenParams>(), Ok(expected));
    assert_eq!(
        "include%2544eleted=true".parse::<DenParams>(),
        Err(DenParamsError::Unknown)
    );
}

#[test]
fn an_unknown_key_is_unknown() {
    for query in [
        "nope=1",
        "includedeleted=true",
        "IncludeDeleted=true",
        "include_deleted=true",
        "=true",
        "%FF=true",
        "%zz=true",
    ] {
        assert_eq!(
            query.parse::<DenParams>(),
            Err(DenParamsError::Unknown),
            "{query}"
        );
    }
}

#[test]
fn an_unknown_key_wins_over_a_bad_value_wherever_it_sits() {
    assert_eq!(
        "includeDeleted=yes&nope=1".parse::<DenParams>(),
        Err(DenParamsError::Unknown)
    );
    assert_eq!(
        "pageToken=00&pageToken=00&nope".parse::<DenParams>(),
        Err(DenParamsError::Unknown)
    );
}

#[test]
fn include_deleted_is_exactly_true_or_false() {
    for query in [
        "includeDeleted",
        "includeDeleted=",
        "includeDeleted=yes",
        "includeDeleted=1",
        "includeDeleted=True",
        "includeDeleted=%74rue%FF",
        "includeDeleted=%zz",
    ] {
        assert_eq!(
            query.parse::<DenParams>(),
            Err(DenParamsError::Malformed),
            "{query}"
        );
    }
}

#[test]
fn a_page_token_the_application_refuses_is_malformed() {
    for query in [
        "pageToken",
        "pageToken=",
        "pageToken=nope",
        "pageToken=0200000000000003e8",
        "pageToken=0100000000000003E8",
    ] {
        assert_eq!(
            query.parse::<DenParams>(),
            Err(DenParamsError::Malformed),
            "{query}"
        );
    }
}

#[test]
fn a_repeated_parameter_is_refused() {
    let repeated_token = format!("pageToken={TOKEN}&pageToken={TOKEN}");

    assert_eq!(
        repeated_token.parse::<DenParams>(),
        Err(DenParamsError::Repeated)
    );
    assert_eq!(
        "includeDeleted=false&includeDeleted=false".parse::<DenParams>(),
        Err(DenParamsError::Repeated)
    );
}
