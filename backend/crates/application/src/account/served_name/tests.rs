use chrono::Utc;
use domain::elements::{account::AccountName, did::Did, user::UserId};

use super::*;

/// The handle every Account here is reached by.
const HANDLE: &str = "served.zurfur.app";

/// An Account under [`HANDLE`] whose stored name is `stored`, a name stored before founding
/// checked it.
fn account_named(stored: &str) -> Account {
    let owner = UserId::from(Did::from("did:plc:servedowneraaaaaaaaaaaaa".to_owned()));
    let did = Did::from("did:plc:servedaccountaaaaaaaaaa".to_owned());
    let handle = HANDLE.parse().expect("a valid handle");
    let name = stored.parse::<AccountName>().expect("a storable name");
    let (account, _owner) = Account::open(owner, did, handle, name, Utc::now());
    account
}

/// What `stored` is served as.
fn served(stored: &str) -> String {
    let account = account_named(stored);
    ServedName::from(&account).to_string()
}

#[test]
fn a_name_the_rule_accepts_unchanged_is_served_as_stored() {
    assert_eq!(served("Supreme Arts"), "Supreme Arts");
    assert_eq!(served("AC/DC Studio"), "AC/DC Studio");
}

#[test]
fn a_name_the_rule_strips_is_served_in_its_kept_form() {
    assert_eq!(served("Ref\u{200B}sheet"), "Refsheet");
}

#[test]
fn a_name_the_rule_refuses_is_served_as_the_handle() {
    let refused = [
        "Studio\u{202E}gnp.exe",
        "Ref\u{2028}sheet",
        "Ref\u{FFFC}sheet",
        "\u{200D}",
        "Ref\u{0007}sheet",
    ];

    let served: Vec<String> = refused.iter().map(|stored| served(stored)).collect();

    let every_one_the_handle = vec![HANDLE.to_owned(); refused.len()];
    assert_eq!(served, every_one_the_handle);
}

#[test]
fn the_served_name_reads_as_its_text() {
    let account = account_named("Ref\u{200B}sheet");

    let served = ServedName::from(&account);

    assert_eq!(served.as_ref(), "Refsheet");
}
