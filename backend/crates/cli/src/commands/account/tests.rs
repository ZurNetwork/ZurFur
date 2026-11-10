use domain::elements::did::Did;

use super::*;

#[test]
fn the_delete_prompt_says_what_happens_either_way() {
    let account_id = AccountId::from(Did::from("did:plc:prompted".to_string()));

    let prompt = delete_prompt(&account_id);

    let expected = "Delete account did:plc:prompted? If it ever had a member besides its \
                    founder, it is switched off and kept, and its handle stays reserved. \
                    Otherwise it is removed for good: its handle is freed and its did:plc \
                    tombstoned, which can't be undone after the PLC recovery window. [y/N] ";
    assert_eq!(prompt, expected);
}
