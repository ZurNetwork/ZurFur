use super::*;

#[test]
fn a_browser_binding_never_prints_its_token() {
    let token = "a-secret-browser-token";
    let binding = BrowserBinding::from(token.to_owned());
    let rendered = format!("{binding:?} {binding:#?}");
    assert!(!rendered.contains(token), "the token leaked: {rendered}");
    assert_eq!(
        binding.as_ref(),
        token,
        "the value is still readable on purpose"
    );
}
