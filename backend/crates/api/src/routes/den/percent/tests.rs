use super::*;

fn decoded(text: &str) -> Result<String, NotPercentEncoded> {
    text.parse::<PercentDecoded>().map(String::from)
}

#[test]
fn plain_text_is_left_as_it_is() {
    assert_eq!(
        decoded("did:plc:abc-1.2_3~"),
        Ok("did:plc:abc-1.2_3~".to_owned())
    );
    assert_eq!(decoded(""), Ok(String::new()));
}

#[test]
fn each_escape_is_decoded_once() {
    assert_eq!(decoded("a%2Fb"), Ok("a/b".to_owned()));
    assert_eq!(decoded("a%2fb"), Ok("a/b".to_owned()));
    assert_eq!(decoded("%252F"), Ok("%2F".to_owned()));
    assert_eq!(decoded("%C3%A9"), Ok("\u{e9}".to_owned()));
}

#[test]
fn a_plus_stays_a_plus() {
    assert_eq!(decoded("a+b"), Ok("a+b".to_owned()));
}

#[test]
fn a_broken_escape_is_refused() {
    for broken in ["%", "%2", "%zz", "a%g1", "%2%41"] {
        assert_eq!(decoded(broken), Err(NotPercentEncoded), "{broken}");
    }
}

#[test]
fn bytes_that_are_not_utf8_are_refused() {
    assert_eq!(decoded("%FF"), Err(NotPercentEncoded));
    assert_eq!(decoded("%C3"), Err(NotPercentEncoded));
}
