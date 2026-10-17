use domain::elements::{
    account::AccountId,
    character::CharacterId,
    commission::{CommissionId, ElementId, FileKey},
    did::Did,
    user::UserId,
    workflow::WorkflowId,
};
use uuid::Uuid;

use super::*;

fn did(text: &str) -> Did {
    Did::from(format!("did:plc:{text}"))
}

fn uuid(last: u128) -> Uuid {
    Uuid::from_u128(last)
}

/// Every place, each built around the ids `user`, `account`, `character`, `commission` and
/// `workflow` given.
fn places(tag: &str, number: u128) -> Vec<Place> {
    let user = UserId::from(did(&format!("user-{tag}")));
    let account = AccountId::from(did(&format!("account-{tag}")));
    let character = CharacterId::from(did(&format!("character-{tag}")));
    let commission = CommissionId::from(uuid(number));
    let workflow = WorkflowId::from(uuid(number + 100));
    vec![
        Place::User(UserPlace::User(user.clone())),
        Place::User(UserPlace::Accounts(user.clone())),
        Place::User(UserPlace::Characters(user.clone())),
        Place::Account(AccountPlace::Account(account.clone())),
        Place::Account(AccountPlace::Characters(account.clone())),
        Place::Character(CharacterPlace::Character(character)),
        Place::Commission(CommissionPlace::UserCommissions(user)),
        Place::Commission(CommissionPlace::AccountCommissions(account.clone())),
        Place::Commission(CommissionPlace::Commission(commission)),
        Place::Commission(CommissionPlace::Attachments(commission)),
        Place::Commission(CommissionPlace::Changelog(commission)),
        Place::Commission(CommissionPlace::Products(commission)),
        Place::Commission(CommissionPlace::Slots(commission)),
        Place::Commission(CommissionPlace::Slot(
            commission,
            ElementId::from(uuid(number + 200)),
        )),
        Place::Commission(CommissionPlace::File(
            commission,
            FileKey::from(uuid(number + 300)),
        )),
        Place::Workflow(WorkflowPlace::Workflows(account.clone())),
        Place::Workflow(WorkflowPlace::Workflow(account, workflow)),
        Place::Post(PostPlace::Posts(did(&format!("poster-{tag}")))),
    ]
}

/// Every top-level directory's place.
fn top_levels() -> Vec<Place> {
    [
        TopLevel::User,
        TopLevel::Account,
        TopLevel::Character,
        TopLevel::Commission,
        TopLevel::Post,
    ]
    .into_iter()
    .map(Place::TopLevel)
    .collect()
}

/// The children each folder holds, by the type of the folder and of the child.
fn holds(parent: &Parent, child: &Place) -> bool {
    let Parent::At(parent) = parent else {
        return child.den_type() == DenType::TopLevel;
    };
    let pair = (parent.den_type(), child.den_type());
    match parent {
        Place::TopLevel(top_level) => child.top_level() == Some(*top_level),
        Place::User(_)
        | Place::Account(_)
        | Place::Character(_)
        | Place::Commission(_)
        | Place::Workflow(_)
        | Place::Post(_) => [
            (DenType::User, DenType::UserAccounts),
            (DenType::User, DenType::UserCharacters),
            (DenType::User, DenType::UserCommissions),
            (DenType::Account, DenType::AccountCharacters),
            (DenType::Account, DenType::AccountCommissions),
            (DenType::Account, DenType::AccountWorkflows),
            (DenType::Commission, DenType::CommissionAttachments),
            (DenType::Commission, DenType::CommissionChangelog),
            (DenType::Commission, DenType::CommissionProducts),
            (DenType::Commission, DenType::CommissionSlots),
            (DenType::CommissionAttachments, DenType::File),
            (DenType::CommissionSlots, DenType::Slot),
            (DenType::AccountWorkflows, DenType::Workflow),
        ]
        .contains(&pair),
    }
}

#[test]
fn the_global_root_holds_only_the_top_level_directories() {
    for child in places("a", 1).into_iter().chain(top_levels()) {
        let expected = matches!(child, Place::TopLevel(_));

        assert_eq!(child.is_child_of(&Parent::Root), expected, "{child:?}");
    }
}

#[test]
fn each_folder_holds_only_its_own_entries() {
    let mine = places("a", 1);
    for parent in mine.iter().chain(top_levels().iter()) {
        let folder = Parent::At(parent.clone());
        for child in &mine {
            let expected = holds(&folder, child);

            let held = child.is_child_of(&folder);

            assert_eq!(held, expected, "{child:?} in {parent:?}");
        }
    }
}

#[test]
fn no_folder_holds_another_entity_s_entries() {
    let mine = places("a", 1);
    let theirs = places("b", 1000);
    for parent in &mine {
        let folder = Parent::At(parent.clone());
        for child in &theirs {
            let held = child.is_child_of(&folder);

            assert!(!held, "{child:?} in {parent:?}");
        }
    }
}

#[test]
fn every_place_has_a_key_and_only_entities_are_anchored() {
    for place in places("a", 1) {
        let anchored = place.anchored_path();

        assert!(place.key().is_some(), "{place:?}");
        assert_eq!(anchored.is_some(), place.top_level().is_some(), "{place:?}");
    }
}

#[test]
fn an_account_s_directory_stands_for_its_account() {
    let account = AccountId::from(did("account-a"));
    let place = Place::Account(AccountPlace::Account(account.clone()));
    let expected_thing = Some(DenThing::Account(account));

    assert_eq!(place.thing(), expected_thing);
}

#[test]
fn a_commission_s_directory_stands_for_its_commission() {
    let commission = CommissionId::from(uuid(1));
    let place = Place::Commission(CommissionPlace::Commission(commission));
    let expected_thing = Some(DenThing::Commission(commission));

    assert_eq!(place.thing(), expected_thing);
}

#[test]
fn every_other_place_stands_for_nothing() {
    let others = places("a", 1)
        .into_iter()
        .chain(top_levels())
        .filter(|place| ![DenType::Account, DenType::Commission].contains(&place.den_type()));
    for place in others {
        assert_eq!(place.thing(), None, "{place:?}");
    }
}
