use super::*;
use crate::generated::{CreateCommissionRequest, CreateCommissionResponse};
use std::path::{Path, PathBuf};
use strum::VariantArray;

fn two_refusals() -> (FieldRefusal, FieldRefusal) {
    let title_refusal = FieldRefusal {
        pointer: FieldPointer::member("title"),
        code: FieldCode::NameEmpty,
    };
    let channel_refusal = FieldRefusal {
        pointer: FieldPointer::member("linkedChannel"),
        code: FieldCode::NameEmpty,
    };
    (title_refusal, channel_refusal)
}

/// The member a pointer names: the pointer without its `#/`.
fn member_of(pointer: &FieldPointer) -> &str {
    pointer
        .as_ref()
        .strip_prefix("#/")
        .expect("a pointer starts with #/")
}

#[test]
fn a_member_pointer_is_written_in_fragment_form() {
    let pointer = FieldPointer::member("title");

    assert_eq!(pointer.to_string(), "#/title");
    assert_eq!(pointer.as_ref() as &str, "#/title");
}

#[test]
fn a_two_word_member_points_at_its_proto_json_name() {
    let response = CreateCommissionResponse {
        title: "A ref sheet".to_owned(),
        linked_channel: Some("https://t.me/example".to_owned()),
        ..Default::default()
    };
    let wire = serde_json::to_value(response).expect("serializes");
    let pointer = FieldPointer::member("linkedChannel");

    let object = wire.as_object().expect("a JSON object");
    assert!(
        object.contains_key(member_of(&pointer)),
        "the wire has no member {:?}; it has {:?}",
        member_of(&pointer),
        object.keys().collect::<Vec<_>>(),
    );
}

#[test]
fn the_title_pointer_names_a_member_of_the_create_request() {
    let request = CreateCommissionRequest {
        title: "A ref sheet".to_owned(),
        ..Default::default()
    };
    let wire = serde_json::to_value(request).expect("serializes");
    let pointer = FieldPointer::member("title");

    let object = wire.as_object().expect("a JSON object");
    assert!(
        object.contains_key(member_of(&pointer)),
        "the request has no member {:?}; it has {:?}",
        member_of(&pointer),
        object.keys().collect::<Vec<_>>(),
    );
}

#[test]
fn invalid_fields_lists_every_refusal_in_order() {
    let (title_refusal, channel_refusal) = two_refusals();

    let problem = Problem::invalid_fields(title_refusal, [channel_refusal]);

    let expected_errors = vec![
        FieldError {
            pointer: "#/title".to_owned(),
            code: "name_empty".to_owned(),
            detail: "This can't be blank.".to_owned(),
        },
        FieldError {
            pointer: "#/linkedChannel".to_owned(),
            code: "name_empty".to_owned(),
            detail: "This can't be blank.".to_owned(),
        },
    ];
    assert_eq!(problem.errors, expected_errors);
    assert_eq!(problem.status, 422);
    assert_eq!(problem.code, "invalid_request");
    assert_eq!(problem.r#type, "urn:zurfur:error:invalid-request");
}

#[test]
fn invalid_fields_rides_on_invalid_request_with_a_fixed_detail() {
    let (title_refusal, _) = two_refusals();
    let invalid_request = Problem::invalid_request("Any detail.");
    let fixed_detail = "Some fields need changing; each one says why.";

    let problem = Problem::invalid_fields(title_refusal, []);

    assert_eq!(problem.r#type, invalid_request.r#type);
    assert_eq!(problem.code, invalid_request.code);
    assert_eq!(problem.title, invalid_request.title);
    assert_eq!(problem.status, invalid_request.status);
    assert_eq!(problem.detail, fixed_detail);
}

#[test]
fn each_code_has_its_own_fixed_detail() {
    let details: Vec<&str> = FieldCode::VARIANTS
        .iter()
        .map(|code| code.detail())
        .collect();

    assert!(details.iter().all(|detail| !detail.is_empty()));
    let mut distinct = details.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), details.len(), "two codes share a detail");
}

#[test]
fn each_code_s_detail_is_these_words() {
    let details: Vec<(&'static str, &'static str)> = FieldCode::VARIANTS
        .iter()
        .map(|code| (<&'static str>::from(*code), code.detail()))
        .collect();

    let expected_details = [
        ("name_empty", "This can't be blank."),
        ("name_too_long", "This is too long. Shorten it."),
        ("name_slash", "Leave out “/”."),
        (
            "name_invisible",
            "Leave out invisible and direction-changing characters.",
        ),
        ("name_separator", "Keep it on one line."),
        ("name_reserved", "Leave out characters that can't be shown."),
    ];
    assert_eq!(details, expected_details);
}

#[test]
fn each_name_rule_refusal_has_its_own_code() {
    let codes: Vec<FieldCode> = [
        DenNameError::Empty,
        DenNameError::TooLong,
        DenNameError::Slash,
        DenNameError::Invisible,
        DenNameError::Separator,
        DenNameError::Reserved,
    ]
    .into_iter()
    .map(FieldCode::from)
    .collect();

    let expected_codes = [
        FieldCode::NameEmpty,
        FieldCode::NameTooLong,
        FieldCode::NameSlash,
        FieldCode::NameInvisible,
        FieldCode::NameSeparator,
        FieldCode::NameReserved,
    ];
    assert_eq!(codes, expected_codes);
}

#[test]
fn field_codes_are_these_tokens() {
    let tokens: Vec<&'static str> = FieldCode::VARIANTS
        .iter()
        .map(|code| <&'static str>::from(*code))
        .collect();
    let displayed: Vec<String> = FieldCode::VARIANTS
        .iter()
        .map(|code| code.to_string())
        .collect();

    let expected_tokens = [
        "name_empty",
        "name_too_long",
        "name_slash",
        "name_invisible",
        "name_separator",
        "name_reserved",
    ];
    assert_eq!(tokens, expected_tokens);
    assert_eq!(displayed, expected_tokens);
}

/// A source file the scans read: its path under `src/` and its text.
struct ScannedFile {
    relative: PathBuf,
    text: String,
}

/// Names each `.rs` file under `dir`, recursively, outside the registry.
fn scanned_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let registry = src.join("problem").join("fields.rs");
    let registry_dir = src.join("problem").join("fields");
    let generated = src.join("generated");
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path == generated || path == registry_dir || path == registry {
            continue;
        }
        if path.is_dir() {
            scanned_paths(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Reads every `.rs` file under `src/` outside the registry and the generated code.
fn scanned_files() -> Vec<ScannedFile> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    scanned_paths(&src, &mut paths);
    assert!(
        paths.len() > 10,
        "the scan found only {} files",
        paths.len()
    );
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", path.display()));
            let relative = path
                .strip_prefix(&src)
                .expect("a scanned file sits under src")
                .to_path_buf();
            ScannedFile { relative, text }
        })
        .collect()
}

/// Every offending line `rule` finds in `files`, written `path:line`.
fn offenders(files: &[ScannedFile], rule: impl Fn(&ScannedFile) -> Vec<usize>) -> Vec<String> {
    files
        .iter()
        .flat_map(|file| {
            rule(file)
                .into_iter()
                .map(|line| format!("{}:{line}", file.relative.display()))
        })
        .collect()
}

/// Whether `byte` can sit inside an identifier: an ASCII letter, digit or `_`.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether `line` is a `//` comment line, `///` and `//!` docs included.
fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Each line of `text` that isn't a comment line, with its 1-based number.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .filter(|(_, line)| !is_comment_line(line))
}

/// Rule A: the code lines of `text` that name `FieldError`.
fn field_error_lines(text: &str) -> Vec<usize> {
    code_lines(text)
        .filter(|(_, line)| line.contains("FieldError"))
        .map(|(number, _)| number)
        .collect()
}

/// `Problem::new`'s empty field-error list, the one `errors` line `problem.rs` may hold.
const EMPTY_ERRORS_LINE: &str = "errors: Vec::new(),";

/// Whether `line` holds the whole word `errors` other than as a module path
/// (`errors::…` or `mod errors`).
fn uses_errors_word(line: &str) -> bool {
    line.match_indices("errors").any(|(start, word)| {
        let before = &line[..start];
        let after = &line[start + word.len()..];
        let whole_word = !before.bytes().next_back().is_some_and(is_ident_byte)
            && !after.bytes().next().is_some_and(is_ident_byte);
        let module_path = after.starts_with("::") || before.ends_with("mod ");
        whole_word && !module_path
    })
}

/// Rule B: the code lines of the file at `relative` (under `src/`) that use the
/// word `errors` other than as a module path. In `problem.rs` the trimmed line
/// [`EMPTY_ERRORS_LINE`] is allowed.
fn errors_word_lines(relative: &Path, text: &str) -> Vec<usize> {
    let in_problem_module = relative == Path::new("problem.rs");
    code_lines(text)
        .filter(|(_, line)| !(in_problem_module && line.trim() == EMPTY_ERRORS_LINE))
        .filter(|(_, line)| uses_errors_word(line))
        .map(|(number, _)| number)
        .collect()
}

/// A file's code lines joined with newlines, remembering where each one came from.
struct CodeText {
    joined: String,
    /// Each joined line's starting byte offset and its 1-based source line number.
    starts: Vec<(usize, usize)>,
}

impl From<&str> for CodeText {
    /// Drops the comment lines of a file's text and joins the rest.
    fn from(text: &str) -> Self {
        let mut joined = String::new();
        let mut starts = Vec::new();
        for (number, line) in code_lines(text) {
            if !starts.is_empty() {
                joined.push('\n');
            }
            starts.push((joined.len(), number));
            joined.push_str(line);
        }
        Self { joined, starts }
    }
}

impl CodeText {
    /// The source line number of the byte at `offset` in the joined text.
    fn line_of(&self, offset: usize) -> usize {
        let index = self.starts.partition_point(|(start, _)| *start <= offset) - 1;
        self.starts[index].1
    }
}

/// The member name when `call` starts with `("name")`: one string literal of
/// ASCII letters and digits, with optional whitespace and trailing comma.
fn literal_member_name(call: &str) -> Option<&str> {
    let quoted = call.strip_prefix('(')?.trim_start().strip_prefix('"')?;
    let name_len = quoted.bytes().take_while(u8::is_ascii_alphanumeric).count();
    let (name, rest) = quoted.split_at(name_len);
    let rest = rest.strip_prefix('"')?.trim_start();
    let closing = rest.strip_prefix(',').unwrap_or(rest).trim_start();
    (!name.is_empty() && closing.starts_with(')')).then_some(name)
}

/// Whether the text around a `::member` segment reads `FieldPointer::member("name")`.
fn is_literal_field_pointer_call(before: &str, after: &str) -> bool {
    let names_field_pointer = before
        .strip_suffix("FieldPointer")
        .is_some_and(|path| !path.bytes().next_back().is_some_and(is_ident_byte));
    names_field_pointer && literal_member_name(after).is_some()
}

/// Rule C: the lines of `text` where a `::member` path segment starts that
/// isn't `FieldPointer::member` called with one literal name. Comment lines
/// are dropped first, so a call may span lines.
fn non_literal_member_lines(text: &str) -> Vec<usize> {
    let code = CodeText::from(text);
    code.joined
        .match_indices("::member")
        .filter(|(start, segment)| {
            let before = &code.joined[..*start];
            let after = &code.joined[start + segment.len()..];
            let member_segment = !after.bytes().next().is_some_and(is_ident_byte);
            member_segment && !is_literal_field_pointer_call(before, after)
        })
        .map(|(start, _)| code.line_of(start))
        .collect()
}

/// A path for snippets that stand in for any scanned file but `problem.rs`.
fn elsewhere() -> &'static Path {
    Path::new("routes/commissions/create.rs")
}

#[test]
fn naming_field_error_in_code_breaks_rule_a() {
    let snippet = "let error = FieldError::default();";
    let first_line = vec![1];

    assert_eq!(field_error_lines(snippet), first_line);
}

#[test]
fn reaching_a_problems_errors_breaks_rule_b() {
    let snippets = [
        r#".ok_or_else(|| Problem { errors: serde_json::from_value(json!([])).unwrap_or_default(), ..Problem::invalid_request("x") })"#,
        r#"Problem { errors, ..Problem::invalid_request("x") }"#,
        r#"serde_json::from_value::<Problem>(json!({"errors": []}))"#,
        "problem.errors.push(e)",
    ];
    let first_line = vec![1];

    for snippet in snippets {
        assert_eq!(
            errors_word_lines(elsewhere(), snippet),
            first_line,
            "rule B should flag {snippet:?}"
        );
    }
}

#[test]
fn a_member_pointer_built_any_other_way_breaks_rule_c() {
    let snippets = [
        r#"FieldPointer::member(String::from("title").leak())"#,
        r#"FieldPointer::member("title".to_owned().leak())"#,
        r#"std::iter::once("title").map(FieldPointer::member)"#,
        r#"P::member("title")"#,
        r#"<FieldPointer>::member("title")"#,
        "Self::member(name)",
    ];
    let first_line = vec![1];

    for snippet in snippets {
        assert_eq!(
            non_literal_member_lines(snippet),
            first_line,
            "rule C should flag {snippet:?}"
        );
    }
}

#[test]
fn comments_module_paths_and_literal_pointers_pass_every_rule() {
    let snippets = [
        "// A blank title never reaches problem.errors by hand.",
        "/// problem.errors",
        "//! errors",
        "    /// FieldError and FieldPointer::member(name)",
        "mod errors;",
        "use super::{errors::read_problem, params::DenParams};",
        "Problem::member_not_found()",
        r#"FieldPointer::member("title")"#,
        r#"crate::problem::FieldPointer::member("linkedChannel")"#,
        "FieldPointer::member(\n    \"title\"\n)",
        "FieldPointer::member(\n    \"title\",\n)",
    ];
    let no_lines: Vec<usize> = Vec::new();

    for snippet in snippets {
        assert_eq!(
            field_error_lines(snippet),
            no_lines,
            "rule A flagged {snippet:?}"
        );
        assert_eq!(
            errors_word_lines(elsewhere(), snippet),
            no_lines,
            "rule B flagged {snippet:?}"
        );
        assert_eq!(
            non_literal_member_lines(snippet),
            no_lines,
            "rule C flagged {snippet:?}"
        );
    }
}

#[test]
fn the_empty_errors_line_passes_only_in_problem_rs() {
    let snippet = "            errors: Vec::new(),";
    let no_lines: Vec<usize> = Vec::new();
    let first_line = vec![1];

    assert_eq!(
        errors_word_lines(Path::new("problem.rs"), snippet),
        no_lines
    );
    assert_eq!(errors_word_lines(elsewhere(), snippet), first_line);
}

#[test]
fn problem_rs_allows_no_other_errors_line() {
    let snippet = "errors: vec![smuggled],";
    let first_line = vec![1];

    assert_eq!(
        errors_word_lines(Path::new("problem.rs"), snippet),
        first_line
    );
}

#[test]
fn an_offender_is_reported_at_its_source_line() {
    let snippet = "// A comment line.\nlet a = 1;\n\n/// Docs.\nlet p = FieldPointer::member(\n    name\n);\nproblem.errors.clear();";
    let member_line = vec![5];
    let errors_line = vec![8];

    assert_eq!(non_literal_member_lines(snippet), member_line);
    assert_eq!(errors_word_lines(elsewhere(), snippet), errors_line);
}

/// Field errors are built only through `Problem::invalid_fields`, so a refusal
/// can't be empty and its detail can't echo input. No code line outside the
/// registry and the generated code names `FieldError` or the word `errors`.
#[test]
fn only_the_registry_builds_field_errors() {
    let files = scanned_files();

    let names_field_error = offenders(&files, |file| field_error_lines(&file.text));
    let uses_errors = offenders(&files, |file| errors_word_lines(&file.relative, &file.text));

    assert!(
        names_field_error.is_empty(),
        "rule A: only Problem::invalid_fields names FieldError:\n  {}",
        names_field_error.join("\n  "),
    );
    assert!(
        uses_errors.is_empty(),
        "rule B: only Problem::invalid_fields fills a problem's errors:\n  {}",
        uses_errors.join("\n  "),
    );
}

/// A field pointer names a member by a string literal, so it is fixed in the
/// code and can never carry what the caller sent.
#[test]
fn field_pointers_name_literal_members() {
    let files = scanned_files();

    let built_otherwise = offenders(&files, |file| non_literal_member_lines(&file.text));

    assert!(
        built_otherwise.is_empty(),
        "rule C: a member pointer is FieldPointer::member(\"name\") with a literal name:\n  {}",
        built_otherwise.join("\n  "),
    );
}
