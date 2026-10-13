use super::*;

fn pieces(raw_path: &str) -> Result<Vec<String>, NotDenPieces> {
    raw_path.parse::<DenPieces>().map(Vec::from)
}

#[test]
fn an_empty_path_is_no_pieces() {
    assert_eq!(pieces(""), Ok(Vec::new()));
}

#[test]
fn the_path_is_split_on_slashes() {
    let expected = vec!["accounts".to_owned(), "did:plc:abc".to_owned()];

    assert_eq!(pieces("accounts/did:plc:abc"), Ok(expected));
}

#[test]
fn empty_pieces_are_kept_for_the_application_to_refuse() {
    let trailing = vec!["accounts".to_owned(), String::new()];
    let doubled = vec!["a".to_owned(), String::new(), "b".to_owned()];

    assert_eq!(pieces("accounts/"), Ok(trailing));
    assert_eq!(pieces("a//b"), Ok(doubled));
}

#[test]
fn an_encoded_slash_stays_inside_its_piece() {
    let expected = vec!["accounts".to_owned(), "a/b".to_owned()];

    assert_eq!(pieces("accounts/a%2Fb"), Ok(expected));
}

#[test]
fn each_piece_is_decoded_exactly_once() {
    let expected = vec!["accounts".to_owned(), "a%2Fb".to_owned()];

    assert_eq!(pieces("accounts/a%252Fb"), Ok(expected));
}

#[test]
fn a_piece_that_is_not_utf8_is_refused() {
    assert_eq!(pieces("accounts/%FF"), Err(NotDenPieces));
    assert_eq!(pieces("accounts/%2"), Err(NotDenPieces));
}

#[test]
fn past_seventeen_pieces_the_rest_is_dropped_undecoded() {
    let raw_path = format!("{}%FF", "a/".repeat(10_000));

    let decoded = pieces(&raw_path).expect("only the first 17 pieces are decoded");

    assert_eq!(decoded, vec!["a".to_owned(); 17]);
}

#[test]
fn a_bad_piece_within_the_first_seventeen_is_refused() {
    let raw_path = format!("{}%FF/a", "a/".repeat(16));

    assert_eq!(pieces(&raw_path), Err(NotDenPieces));
}
