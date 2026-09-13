use super::*;
use serde_json::json;

#[test]
fn compact_is_one_line_newline_terminated() {
    let out = Output::json(json!({"b": 1, "a": [1, 2]}), Format::Compact);
    assert_eq!(out.to_bytes(), b"{\"a\":[1,2],\"b\":1}\n");
}

#[test]
fn pretty_is_indented_and_newline_terminated() {
    let out = Output::json(json!({"a": 1}), Format::Pretty);
    assert_eq!(out.to_bytes(), b"{\n  \"a\": 1\n}\n");
}

#[test]
fn the_flag_selects_compact() {
    assert_eq!(Format::from_flag(true), Format::Compact);
    assert_eq!(Format::from_flag(false), Format::Pretty);
}
