//! The contract-drift gate, mirroring
//! `adapter-pg`'s `codegen_current`: regenerate the contract module into a
//! temp dir with the SAME generation body `just gen-contract` uses, and diff
//! it against the committed `src/generated/`. A corpus edit without a
//! regenerate — or a hand-edit to the `@generated` files — fails here with
//! the offending file named, never at a reviewer's discretion.

use std::collections::BTreeSet;
use std::path::Path;

/// Every file the generator owns. A new generated file must be added here —
/// deliberately, so the committed set is enumerated rather than discovered.
const GENERATED: &[&str] = &["mod.rs", "zurfur.api.v1.rs", "zurfur.api.v1.serde.rs"];

#[test]
fn committed_contract_module_is_current() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract_dir = manifest.join("../../../contract");
    let committed_dir = manifest.join("src/generated");

    let fresh = tempfile::tempdir().expect("temp dir");
    contract_gen::generate(&contract_dir, fresh.path()).expect("generation succeeds");

    for name in GENERATED {
        let fresh_bytes = std::fs::read(fresh.path().join(name))
            .unwrap_or_else(|e| panic!("{name}: fresh generation missing ({e})"));
        let committed_bytes = std::fs::read(committed_dir.join(name)).unwrap_or_else(|e| {
            panic!("{name}: committed file missing ({e}) — run `just gen-contract`")
        });
        assert!(
            fresh_bytes == committed_bytes,
            "src/generated/{name} is stale: the corpus and the committed module \
             disagree. Run `just gen-contract` and commit the result — the \
             generated files are never hand-edited."
        );
    }

    // And nothing extra lives in the committed dir: a file the generator does
    // not own would silently survive regeneration forever.
    for entry in std::fs::read_dir(&committed_dir).expect("read committed dir") {
        let name = entry.expect("entry").file_name();
        let name = name.to_string_lossy().into_owned();
        assert!(
            GENERATED.contains(&name.as_str()),
            "src/generated/{name} is not a generator-owned file — the generated \
             tree carries only what `just gen-contract` writes"
        );
    }
}

/// Every `.proto` in the v1 corpus is a generation source: a file left out of
/// `contract_gen::SOURCES` would ship to the web while the backend never sees
/// its types.
#[test]
fn every_corpus_file_is_a_generation_source() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contract_dir = manifest.join("../../../contract");
    let corpus_dir = contract_dir.join("zurfur/api/v1");

    let corpus_files: BTreeSet<String> = std::fs::read_dir(&corpus_dir)
        .expect("read the v1 corpus")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "proto")
        })
        .map(|path| {
            let relative = path
                .strip_prefix(&contract_dir)
                .expect("a corpus file lives under the contract root");
            relative.to_string_lossy().into_owned()
        })
        .collect();
    let sources: BTreeSet<String> = contract_gen::SOURCES
        .iter()
        .map(|source| source.to_string())
        .collect();

    assert_eq!(
        corpus_files, sources,
        "contract_gen::SOURCES and the .proto files in the v1 corpus disagree"
    );
}
