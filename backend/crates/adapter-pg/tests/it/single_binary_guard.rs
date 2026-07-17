//! Guard: the single-integration-binary convention (see `main.rs`) drifts
//! silently — a file in `tests/it/` without a `mod` line never compiles or
//! runs (coverage just vanishes, nothing fails), and a file directly under
//! `tests/` becomes its own binary with its own container boot. This test
//! makes both loud.

use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn every_test_file_is_a_module_of_this_binary() {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let it_dir = tests_dir.join("it");

    let mut on_disk = BTreeSet::new();
    for entry in std::fs::read_dir(&it_dir).expect("read tests/it") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "rs")
            && let Some(stem) = path.file_stem()
        {
            let stem = stem.to_string_lossy().into_owned();
            if stem != "main" {
                on_disk.insert(stem);
            }
        }
    }

    let main_rs = std::fs::read_to_string(it_dir.join("main.rs")).expect("read main.rs");
    let declared: BTreeSet<String> = main_rs
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("mod ")?
                .strip_suffix(';')
                .map(str::to_string)
        })
        .collect();

    assert_eq!(
        on_disk, declared,
        "tests/it/*.rs and the `mod` list in tests/it/main.rs disagree — a file \
         without a `mod` line silently neither compiles nor runs"
    );

    let strays: Vec<_> = std::fs::read_dir(&tests_dir)
        .expect("read tests/")
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            path.extension()
                .is_some_and(|ext| ext == "rs")
                .then_some(path)
        })
        .collect();
    assert!(
        strays.is_empty(),
        "integration tests belong in tests/it/ as modules of the single binary \
         (a stray tests/*.rs is its own binary and pays its own container boot): {strays:?}"
    );
}
