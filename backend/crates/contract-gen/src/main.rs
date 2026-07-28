//! `just gen-contract` — regenerate the api crate's committed
//! `src/generated/` module from the repo-root contract corpus (DD 40992770
//! decision 11). The workspace build never needs this to run: the output is
//! committed and `@generated`-marked, and the api crate's `contract_current`
//! test fails loudly (with a diff) when it is stale. One generation body
//! lives in [`contract_gen::generate`]; this binary and that test are its two
//! callers.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let contract_dir = manifest.join("../../../contract");
    let out_dir = manifest.join("../api/src/generated");
    contract_gen::generate(&contract_dir, &out_dir)?;
    println!("regenerated {}", out_dir.display());
    Ok(())
}
