//! CI guard: no domain crate's `elements` imports its `ports`.
//!
//! Elements are the pure domain vocabulary; ports are the traits adapters
//! implement against that vocabulary. A `crate::ports` reference inside
//! `elements` would point the dependency the wrong way — the port depending on
//! its own consumer's consumer — so this is a straight source-text scan, not a
//! type-level check: `cargo check` cannot see an import that compiles fine but
//! points the wrong direction on purpose.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::domain_crates_point_one_way::{FACADE, behind_the_facade};

/// The path segment this guard forbids outside comments.
const FORBIDDEN: &str = "ports";

/// Paths allowed to contain [`FORBIDDEN`], as `<crate-relative path>`.
///
/// Empty, and it should stay that way. If one ever lands here it carries its
/// reason inline.
const EXEMPT: &[&str] = &[];

/// Domain crates that must have both `src/elements.rs` and `src/elements/`,
/// as folder names beside this crate's.
const REQUIRED_CRATES: &[&str] = &["kernel", "identity", "workflow"];

/// Domain crates scanned for `src/elements.rs` and `src/elements/` when they
/// have them, as folder names beside this crate's.
const OPTIONAL_CRATES: &[&str] = &["gallery", "moderation"];

/// The files this guard scans: the `src/elements.rs` façade and every leaf
/// under `src/elements/` of each required crate and each optional crate that
/// has them, plus the façade crate's `src/elements.rs`.
///
/// Panics when a required crate lacks one of them, so a moved or flattened
/// layout cannot make the guard pass while checking nothing.
fn scanned_files() -> Vec<PathBuf> {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .to_path_buf();
    let missing = |name: &str, what: &str| {
        format!(
            "guard expects domain crate `{name}` to have {what} — if the crate's layout \
             changed, update the guard's crate list"
        )
    };
    let mut files = Vec::new();
    for name in REQUIRED_CRATES {
        let src = crates.join(name).join("src");
        let leaves = src.join("elements");
        let facade = src.join("elements.rs");
        assert!(leaves.is_dir(), "{}", missing(name, "`src/elements/`"));
        assert!(facade.is_file(), "{}", missing(name, "`src/elements.rs`"));
        files.extend(rust_files(&leaves));
        files.push(facade);
    }
    for name in OPTIONAL_CRATES {
        let src = crates.join(name).join("src");
        let leaves = src.join("elements");
        if leaves.is_dir() {
            files.extend(rust_files(&leaves));
        }
        let facade = src.join("elements.rs");
        if facade.is_file() {
            files.push(facade);
        }
    }
    let facade = crates.join(FACADE).join("src").join("elements.rs");
    assert!(facade.is_file(), "{}", missing(FACADE, "`src/elements.rs`"));
    files.push(facade);
    files
}

/// Every `*.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// The `<crate>/src/<file…>` suffix of a scanned path — the key exemptions and
/// failure messages are stated on, so an exemption cannot leak across crates.
fn crate_relative(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = canon.to_string_lossy().replace('\\', "/");
    match text.rsplit_once("/crates/") {
        Some((_, rel)) => rel.to_string(),
        None => text,
    }
}

/// Whether a code line names the `ports` module as a path segment: `crate::ports`,
/// `self::ports`, `super::ports`, an alias (`ports as p`), the grouped
/// `use crate::{x, ports}` / `{ports::Y}` and a wrapped `    ports,` all count;
/// `supports::`, `ports_len` and `let ports = …` do not.
fn references_ports(line: &str) -> bool {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(FORBIDDEN).any(|(start, word)| {
        let before_text = line[..start].trim_end();
        let before = line[..start].chars().next_back();
        let after = &line[start + word.len()..];
        let rest = after.trim_start();
        let word_start = !before.is_some_and(is_word_char);
        let in_use_group =
            before_text.is_empty() || before_text.ends_with('{') || before_text.ends_with(',');
        let ends_group_item =
            rest.starts_with('}') || rest.starts_with(',') || rest.starts_with("as ");
        let path_end = after.starts_with("::")
            || (before == Some(':') && !after.starts_with(is_word_char))
            || (in_use_group && ends_group_item);
        word_start && path_end
    })
}

/// Every `ports` path reference outside a comment line, as `file:line`.
fn offenders(files: &[PathBuf]) -> Vec<String> {
    let mut hits = Vec::new();
    for file in files {
        let relative = crate_relative(file);
        if EXEMPT.contains(&relative.as_str()) {
            continue;
        }
        let src = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", file.display()));

        for (index, line) in src.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if references_ports(line) {
                hits.push(format!("{relative}:{}", index + 1));
            }
        }
    }
    hits
}

#[test]
fn scan_finds_element_files() {
    let files = scanned_files();
    let facade_scanned = files
        .iter()
        .any(|path| path.ends_with("src/elements.rs") && path.is_file());
    assert!(
        facade_scanned,
        "guard does not scan the src/elements.rs façade — check the path"
    );
    assert!(
        files.len() > 1,
        "guard scanned no files under src/elements — check the path"
    );
}

#[test]
fn elements_never_reference_ports() {
    let files = scanned_files();
    let offenders = offenders(&files);
    assert!(
        offenders.is_empty(),
        "elements reference `{FORBIDDEN}` — ports depend on elements, never the \
         reverse:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn detector_flags_every_import_shape() {
    assert!(references_ports("use crate::ports::Database;"));
    assert!(references_ports("use crate::{elements::x, ports::Y};"));
    assert!(references_ports("    ports::Y,"));
    assert!(references_ports("use super::ports::Y;"));
    assert!(references_ports("use self::inner::ports::Y;"));
    assert!(references_ports("use crate::ports;"));
    assert!(references_ports("use crate::ports as p;"));
    assert!(references_ports("use crate::{ports};"));
    assert!(references_ports("use crate::{elements::x, ports};"));
    assert!(references_ports("use crate::{ports as p, elements::x};"));
    assert!(references_ports("    ports,"));
}

#[test]
fn detector_ignores_lookalikes() {
    assert!(!references_ports("use crate::supports::Y;"));
    assert!(!references_ports("let ports_len = 3;"));
    assert!(!references_ports("let ports = Vec::new();"));
    assert!(!references_ports("fn transports() {}"));
}

#[test]
fn the_scanned_crates_are_the_crates_behind_the_facade() {
    let scanned: BTreeSet<&str> = REQUIRED_CRATES
        .iter()
        .chain(OPTIONAL_CRATES)
        .copied()
        .collect();
    assert_eq!(
        scanned,
        behind_the_facade(),
        "the elements guard's required and optional crates must equal the one-way \
         guard's `ALLOWED` table minus the façade, so a new domain crate goes into one \
         of them"
    );
}
