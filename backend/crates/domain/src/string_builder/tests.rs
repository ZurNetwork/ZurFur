use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Probe(String);

#[derive(Debug, PartialEq, Eq)]
enum ProbeError {
    Empty,
    TooLong { max: usize, len: usize },
    ControlCharacter,
}

impl TryFrom<String> for Probe {
    type Error = ProbeError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(512)
            .no_control()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => ProbeError::Empty,
                StringBuilderViolation::TooLong { max, len } => ProbeError::TooLong { max, len },
                StringBuilderViolation::ControlCharacter => ProbeError::ControlCharacter,
            })
    }
}

// Each rule its own method, no `?` until the finishing `build`.
#[test]
fn a_full_chain_trims_and_builds_into_the_newtype() {
    let probe = Probe::try_from("  hello  ".to_owned()).unwrap();
    assert_eq!(probe.0, "hello");
}

// Only the FIRST failing rule is reported.
#[test]
fn only_the_first_violation_is_reported() {
    let result = StringBuilder::new("   ")
        .trimmed()
        .non_empty()
        .max_chars(1)
        .no_control()
        .build();
    assert_eq!(result, Err(StringBuilderViolation::Empty));
}

#[test]
fn max_chars_reports_the_cap_and_offending_length() {
    let result = StringBuilder::new("hello")
        .trimmed()
        .non_empty()
        .max_chars(3)
        .build();
    assert_eq!(
        result,
        Err(StringBuilderViolation::TooLong { max: 3, len: 5 })
    );
}

#[test]
fn no_control_rejects_any_control_character() {
    let result = StringBuilder::new("a\nb")
        .trimmed()
        .non_empty()
        .no_control()
        .build();
    assert_eq!(result, Err(StringBuilderViolation::ControlCharacter));
}

// The exception list lets line-structured free text through, but not NUL.
#[test]
fn no_control_except_allows_only_the_listed_characters() {
    let allowed = StringBuilder::new("a\nb\tc")
        .trimmed()
        .non_empty()
        .no_control_except(&['\n', '\t'])
        .build()
        .unwrap();
    assert_eq!(allowed, "a\nb\tc");

    let rejected = StringBuilder::new("a\0b")
        .trimmed()
        .non_empty()
        .no_control_except(&['\n', '\t'])
        .build();
    assert_eq!(rejected, Err(StringBuilderViolation::ControlCharacter));
}

// build() is the plain-String exit for a caller with no newtype.
#[test]
fn build_returns_the_plain_rule_applied_string() {
    assert_eq!(
        StringBuilder::new("  hi  ").trimmed().non_empty().build(),
        Ok("hi".to_owned())
    );
    assert_eq!(
        StringBuilder::new("   ").trimmed().non_empty().build(),
        Err(StringBuilderViolation::Empty)
    );
}

// Once a rule fails, later rules — trimmed() included — are structural
// no-ops, and build() reports the first violation.
#[test]
fn once_failed_later_rules_are_structural_no_ops() {
    let first_violation = StringBuilder::new("   ")
        .trimmed()
        .non_empty()
        // max_chars(0) would otherwise report TooLong.
        .trimmed()
        .max_chars(0)
        .no_control()
        .build();
    assert_eq!(first_violation, Err(StringBuilderViolation::Empty));
}
