//! The custody guard: `src/custody.rs` is the one file of composition and the drivers
//! that names the custodian port or its HKDF adapter, so every derivation runs behind
//! the boot guard there. Its own tests are beside it and may name them too.

use std::fs;
use std::path::{Path, PathBuf};

/// What only the custody file may name.
const CUSTODIAN_NAMES: [&str; 3] = ["KeyCustodian", "adapter_hkdf", "HkdfKeyCustodian"];

/// The source folders scanned: composition's own and each driver's.
const SCANNED: [&str; 3] = ["src", "../api/src", "../cli/src"];

/// The custody file and its tests' folder, relative to this crate.
const CUSTODY_FILE: &str = "src/custody.rs";
const CUSTODY_TESTS: &str = "src/custody";

/// Every `.rs` file under `folder`.
fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = fs::read_dir(&next).expect("a source folder reads");
        for entry in entries {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn only_the_custody_file_names_the_custodian() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let custody_file = crate_root.join(CUSTODY_FILE);
    let custody_tests = crate_root.join(CUSTODY_TESTS);
    let mut offenders = Vec::new();
    let mut scanned = 0;

    for folder in SCANNED {
        for file in rust_files(&crate_root.join(folder)) {
            scanned += 1;
            if file == custody_file || file.starts_with(&custody_tests) {
                continue;
            }
            let source = fs::read_to_string(&file).expect("a source file reads");
            let named: Vec<&str> = CUSTODIAN_NAMES
                .into_iter()
                .filter(|name| source.contains(name))
                .collect();
            if !named.is_empty() {
                offenders.push(format!("{} names {named:?}", file.display()));
            }
        }
    }

    assert!(scanned > 10, "the scan read the sources: {scanned} files");
    assert!(offenders.is_empty(), "{offenders:#?}");
}

#[test]
fn the_custody_file_guards_before_it_derives() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join(CUSTODY_FILE)).expect("custody.rs reads");

    let guards: Vec<usize> = source
        .match_indices("ensure_custody_hardened(")
        .map(|(at, _)| at)
        .collect();
    let derivations: Vec<usize> = source
        .match_indices("HkdfKeyCustodian::derive(")
        .map(|(at, _)| at)
        .collect();

    assert_eq!(guards.len(), 1, "one guard call");
    assert_eq!(derivations.len(), 1, "one derivation");
    assert!(guards[0] < derivations[0], "the guard runs first");
}
