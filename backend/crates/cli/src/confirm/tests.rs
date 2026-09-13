use super::*;
use crate::ExitClass;

const PROMPT: &str = "Delete account 0? [y/N] ";

/// Run the gate over a canned answer as if a person were there, and hand
/// back the outcome plus whatever reached the question channel.
fn asked(reply: &str) -> (Result<(), CliError>, String) {
    let mut question = Vec::new();
    let outcome = confirm(
        PROMPT,
        Session::Interactive,
        reply.as_bytes(),
        &mut question,
    );
    (outcome, String::from_utf8(question).unwrap())
}

#[test]
fn a_typed_y_proceeds_and_the_prompt_went_to_the_question_channel() {
    let (outcome, question) = asked("y\n");
    assert!(outcome.is_ok());
    assert_eq!(question, PROMPT);
}

#[test]
fn the_spelled_out_yes_proceeds() {
    assert!(asked("yes\n").0.is_ok());
}

#[test]
fn the_answer_is_case_insensitive_and_trimmed() {
    assert!(asked("Y\n").0.is_ok());
    assert!(asked("  YES  \n").0.is_ok());
}

#[test]
fn a_no_cancels() {
    let error = asked("n\n").0.unwrap_err();
    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "cancelled");
}

#[test]
fn a_bare_enter_cancels_the_default_is_no() {
    assert_eq!(asked("\n").0.unwrap_err().code(), "cancelled");
}

#[test]
fn eof_cancels() {
    assert_eq!(asked("").0.unwrap_err().code(), "cancelled");
}

// "yeah", "ye", "yolo" — near-misses are refusals, not consent.
#[test]
fn an_unrecognized_answer_cancels() {
    assert_eq!(asked("yeah\n").0.unwrap_err().code(), "cancelled");
}

// The whole point: no terminal, no deletion — and the answer stream is
// never even read, so piping `y` cannot stand in for a person.
#[test]
fn a_detached_session_is_refused_without_reading_the_answer() {
    let mut question = Vec::new();
    let error = confirm(PROMPT, Session::Detached, &b"y\n"[..], &mut question)
        .expect_err("a detached session never proceeds");

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "confirmation_required");
    assert!(question.is_empty(), "nothing is asked of nobody");
}
