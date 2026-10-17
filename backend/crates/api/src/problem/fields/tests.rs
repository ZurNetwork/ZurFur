use super::*;
use crate::generated::{CreateCommissionRequest, CreateCommissionResponse};
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
    assert_eq!(FieldCode::NameEmpty.detail(), "This can't be blank.");
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

    assert_eq!(tokens, ["name_empty"]);
    assert_eq!(displayed, ["name_empty"]);
}

/// Names each `.rs` file under `dir`, recursively, outside the registry.
fn scanned_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
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
            scanned_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Field errors are built only through `Problem::invalid_fields`, so a refusal
/// can't be empty and its detail can't echo input. No file outside the registry
/// and the generated code names `FieldError` or reaches a problem's `.errors`.
#[test]
fn only_the_registry_builds_field_errors() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    scanned_files(&src, &mut files);
    assert!(
        files.len() > 10,
        "the scan found only {} files",
        files.len()
    );

    let mut offenders = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", file.display()));
        for (index, line) in text.lines().enumerate() {
            if line.contains("FieldError") || line.contains(".errors") {
                let relative = file.strip_prefix(&src).unwrap_or(file);
                offenders.push(format!("{}:{}", relative.display(), index + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "field errors are built only through Problem::invalid_fields:\n  {}",
        offenders.join("\n  "),
    );
}
