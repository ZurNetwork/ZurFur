//! Generates this crate's typed query registry (`crate::queries`) from the
//! `queries/` tree — see `query-registry-build` for the shape it emits.

use std::{env, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let queries = manifest.join("queries");
    println!("cargo:rerun-if-changed={}", queries.display());

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("queries_gen.rs");
    query_registry_build::generate(&queries, &out);
}
