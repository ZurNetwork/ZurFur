use domain::elements::custody::{CustodyError, MacMessage, MacUse};
use domain::elements::did::Did;

use super::*;

// Only an allowlisted key's own message passes through: any other key's
// message renders the generic detail, so a value it carries never reaches
// stderr.
#[test]
fn a_message_on_a_key_outside_the_allowlist_renders_the_generic_detail() {
    let mut secret_message = figment::Error::from("expected a key, found hunter2".to_owned());
    secret_message.path = vec!["did_key_root_key".to_owned()];
    let load_error = LoadError::Figment(Box::new(secret_message));

    let detail = config_problem(&load_error).problem().detail;

    assert!(detail.contains("could not be loaded"), "{detail}");
    assert!(detail.contains("`did_key_root_key`"), "{detail}");
    assert!(!detail.contains("hunter2"), "{detail}");
}

// `public_url`'s parser names only the rule it broke, so its message is shown.
#[test]
fn a_message_on_an_allowlisted_key_is_shown() {
    let mut rule_message =
        figment::Error::from("`public_url` must be a bare origin: it carries a path".to_owned());
    rule_message.path = vec!["public_url".to_owned()];
    let load_error = LoadError::Figment(Box::new(rule_message));

    let detail = config_problem(&load_error).problem().detail;

    assert_eq!(
        detail,
        "`public_url` must be a bare origin: it carries a path"
    );
}

/// The commands that hold no purpose, one of each namespace.
fn rootless_commands() -> [BackendCommand; 3] {
    let whoami = BackendCommand::Session {
        op: commands::session::SessionOp::Whoami,
    };
    let bug_reports = BackendCommand::BugReports {
        op: commands::bug_reports::BugReportsOp::List { since: None },
    };
    [BackendCommand::Health, whoami, bug_reports]
}

/// A message for the MAC probe.
fn message() -> MacMessage {
    let parts: [&[u8]; 1] = [b"a node"];
    MacMessage::try_from(parts.as_slice()).expect("one short part encodes")
}

// `health`, `session` and `bug-reports list` hold no purpose, so their custody
// is the one that reads no root and answers `PurposeNotHeld`.
#[tokio::test]
async fn health_runtime_holds_no_purpose() {
    let did = Did::from("did:plc:cli-purposes".to_owned());
    let config = test_support::runtime::mem(&did).build().runtime.config;

    for command in rootless_commands() {
        let held = purposes(&command);
        assert_eq!(held, [], "{command:?} holds no purpose");
        let custody = custody(&config, held).expect("no root is read");
        let tagged = custody.den_mac().mac(MacUse::DenNode, &message()).await;
        let not_held = CustodyError::PurposeNotHeld {
            purpose: Purpose::DenMac,
        };
        assert_eq!(tagged.err(), Some(not_held), "{command:?}");
    }
}

// `account` and `migrate` hold DID custody and nothing else.
#[test]
fn the_minting_commands_hold_did_custody_only() {
    let create = BackendCommand::Account {
        op: commands::account::AccountOp::Create {
            name: "Acme".to_owned(),
            handle: "acme.zurfur.app".to_owned(),
        },
    };

    assert_eq!(purposes(&create), [Purpose::DidCustody]);
    assert_eq!(MIGRATE_PURPOSES, [Purpose::DidCustody]);
}
