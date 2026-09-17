//! CI guard: `domain::elements` never imports `domain::ports`.
//!
//! Elements are the pure domain vocabulary; ports are the traits adapters
//! implement against that vocabulary. A `crate::ports` reference inside
//! `elements` would point the dependency the wrong way — the port depending on
//! its own consumer's consumer — so this is a straight source-text scan, not a
//! type-level check: `cargo check` cannot see an import that compiles fine but
//! points the wrong direction on purpose.

use std::path::{Path, PathBuf};

/// The substring this guard forbids outside comments.
const FORBIDDEN: &str = "crate::ports";

/// Paths allowed to contain [`FORBIDDEN`], as `<crate-relative path>`.
///
/// Empty, and it should stay that way. If one ever lands here it carries its
/// reason inline.
const EXEMPT: &[&str] = &[];

/// The `elements` directory this guard scans, relative to this crate's
/// `CARGO_MANIFEST_DIR` (`backend/crates/domain`).
fn scanned_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/elements")
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

/// Every `crate::ports` reference outside a comment line, as `file:line`.
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
            if line.contains(FORBIDDEN) {
                hits.push(format!("{relative}:{}", index + 1));
            }
        }
    }
    hits
}

#[test]
fn scan_finds_element_files() {
    let files = rust_files(&scanned_src_dir());
    assert!(
        !files.is_empty(),
        "guard scanned zero files under src/elements — check the path"
    );
}

#[test]
fn elements_never_reference_ports() {
    let files = rust_files(&scanned_src_dir());
    let offenders = offenders(&files);
    assert!(
        offenders.is_empty(),
        "elements reference `{FORBIDDEN}` — ports depend on elements, never the \
         reverse:\n  {}",
        offenders.join("\n  ")
    );
}
