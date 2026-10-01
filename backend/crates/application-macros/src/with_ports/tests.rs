use super::snake_case;

#[test]
fn a_single_word_is_lowercased() {
    assert_eq!(snake_case("Accounts"), "accounts");
}

#[test]
fn camel_case_words_are_joined_by_underscores() {
    assert_eq!(snake_case("ViewGrants"), "view_grants");
}

#[test]
fn three_words_split_at_each_capital() {
    assert_eq!(snake_case("DeadlineStatusSet"), "deadline_status_set");
}

#[test]
fn an_already_lowercase_name_is_unchanged() {
    assert_eq!(snake_case("facts"), "facts");
}
