//! The source scans' shared walker: which files of a crate are production, the identifiers in
//! them token by token, the aliases a workspace binds to a name, the driver crates as cargo
//! metadata names them, and throwaway fixture trees.

#![allow(
    dead_code,
    reason = "each scan crate that includes these helpers uses its own part of them"
)]

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use syn::{Item, UseTree, ext::IdentExt};

/// The identifiers that reach the node port: a unit of work's accessor, the port's traits, the
/// PostgreSQL adapter's types, and the in-memory adapter's accessor and types.
pub const NODE_PORT: [&str; 10] = [
    "nodes",
    "NodeRepo",
    "NodeStore",
    "NodeReads",
    "NodeWrites",
    "PgNodeStore",
    "PgNodeWrites",
    "node_store",
    "MemNodeStore",
    "MemNodeWrites",
];

/// The names of unit-test and property-test files, which must be declared under `#[cfg(test)]`.
pub const TEST_FILES: [&str; 2] = ["tests.rs", "proptests.rs"];

/// The composition root: it and every crate with a normal dependency on it is a driver.
pub const COMPOSITION: &str = "composition";

/// The test rig. It has a normal dependency on the composition root but is never built into
/// a binary: every crate that names it does so as a dev-dependency, which
/// `the_scan_reads_every_crate_that_depends_on_composition` holds.
pub const TEST_SUPPORT: &str = "test-support";

/// The use-case layer, scanned for the node port beside the drivers but not a driver.
pub const APPLICATION: &str = "application";

/// The folder holding every crate of the workspace.
pub fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .to_path_buf()
}

/// Every identifier in `tokens`, groups and macro bodies included, raw ones without `r#`.
pub fn identifiers(tokens: TokenStream) -> Vec<String> {
    tokens
        .into_iter()
        .flat_map(|tree| match tree {
            TokenTree::Group(group) => identifiers(group.stream()),
            TokenTree::Ident(ident) => vec![ident.unraw().to_string()],
            TokenTree::Punct(_) | TokenTree::Literal(_) => Vec::new(),
        })
        .collect()
}

pub fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| TEST_FILES.contains(&name))
}

/// Whether `items` declare the out-of-line module `name` under `#[cfg(test)]`, at its own path.
pub fn declares_under_cfg_test(items: &[Item], name: &str) -> bool {
    items.iter().any(|item| {
        let Item::Mod(module) = item else {
            return false;
        };
        let under_cfg_test = module.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string() == "test")
        });
        let moved = module
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("path"));
        module.ident == name && module.content.is_none() && under_cfg_test && !moved
    })
}

/// The module file `path` stands for, as its parent names it, and the files that could be
/// that parent: `lib.rs` or `main.rs` for a module at `src`'s top, else the folder's
/// `<folder>.rs` or `mod.rs`. `None` for the crate root.
fn parent_modules(path: &Path, src: &Path) -> Option<(String, Vec<PathBuf>)> {
    let file_name = path.file_name().and_then(|name| name.to_str())?;
    let folder = path.parent()?;
    if folder == src && (file_name == "lib.rs" || file_name == "main.rs") {
        return None;
    }
    let (name, folder) = if file_name == "mod.rs" {
        let module = folder.file_name().and_then(|name| name.to_str())?;
        (module.to_owned(), folder.parent()?)
    } else {
        let stem = path.file_stem().and_then(|stem| stem.to_str())?;
        (stem.to_owned(), folder)
    };
    let parents = if folder == src {
        vec![src.join("lib.rs"), src.join("main.rs")]
    } else {
        vec![folder.with_extension("rs"), folder.join("mod.rs")]
    };
    Some((name, parents))
}

/// Whether the module file at `path` is test-only: its parent module declares it under
/// `#[cfg(test)]`, or that parent is test-only itself. A file no parent declares is not.
pub fn test_only(path: &Path, src: &Path) -> bool {
    let Some((name, parents)) = parent_modules(path, src) else {
        return false;
    };
    let present: Vec<PathBuf> = parents
        .into_iter()
        .filter(|parent| parent.is_file())
        .collect();
    let declared = present.iter().any(|parent| {
        let source = std::fs::read_to_string(parent).expect("a readable parent module");
        let file = syn::parse_file(&source).expect("the parent module parses");
        declares_under_cfg_test(&file.items, &name)
    });
    declared || present.iter().any(|parent| test_only(parent, src))
}

/// One crate's sources: the production files a scan reads, and each test file no parent
/// module declares under `#[cfg(test)]` (read as production too).
#[derive(Default)]
pub struct Walk {
    pub production: Vec<PathBuf>,
    pub undeclared: Vec<PathBuf>,
}

fn walk_folder(folder: &Path, src: &Path, walk: &mut Walk) {
    let entries = std::fs::read_dir(folder).expect("a readable folder");
    for entry in entries {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            walk_folder(&path, src, walk);
        } else if path.extension().is_none_or(|extension| extension != "rs")
            || test_only(&path, src)
        {
            continue;
        } else if is_test_file(&path) {
            walk.undeclared.push(path.clone());
            walk.production.push(path);
        } else {
            walk.production.push(path);
        }
    }
}

/// Every Rust source under `src`, the modules declared under `#[cfg(test)]` (and theirs) skipped.
pub fn walk(src: &Path) -> Walk {
    let mut walk = Walk::default();
    walk_folder(src, src, &mut walk);
    walk.production.sort();
    walk.undeclared.sort();
    walk
}

/// Every name `items` bind to a forbidden one: a renamed import of it, or a type alias naming
/// it. A rename to `_` binds nothing.
pub fn aliases_in(items: &[Item], forbidden: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Use(item_use) => renames_in(&item_use.tree, forbidden, out),
            Item::Type(alias) => {
                let names_forbidden = identifiers(alias.ty.to_token_stream())
                    .iter()
                    .any(|ident| forbidden.contains(ident));
                if names_forbidden {
                    out.insert(alias.ident.to_string());
                }
            }
            Item::Mod(module) => {
                if let Some((_, content)) = &module.content {
                    aliases_in(content, forbidden, out);
                }
            }
            _ => {}
        }
    }
}

fn renames_in(tree: &UseTree, forbidden: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => renames_in(&path.tree, forbidden, out),
        UseTree::Rename(rename) => {
            let bound = rename.rename.to_string();
            if forbidden.contains(&rename.ident.to_string()) && bound != "_" {
                out.insert(bound);
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                renames_in(item, forbidden, out);
            }
        }
        UseTree::Name(_) | UseTree::Glob(_) => {}
    }
}

/// `seeds`, and every name `sources` bind to one of them, through any chain of aliases.
pub fn with_aliases(seeds: &[&str], sources: &[String]) -> BTreeSet<String> {
    let files: Vec<syn::File> = sources
        .iter()
        .map(|source| syn::parse_file(source).expect("the source parses"))
        .collect();
    let mut forbidden: BTreeSet<String> = seeds.iter().map(|name| (*name).to_owned()).collect();
    loop {
        let mut grown = forbidden.clone();
        for file in &files {
            aliases_in(&file.items, &forbidden, &mut grown);
        }
        if grown == forbidden {
            return forbidden;
        }
        forbidden = grown;
    }
}

/// The production files of every crate in the workspace: its `src`, and a build script.
pub fn workspace_production() -> Vec<PathBuf> {
    let entries = std::fs::read_dir(crates()).expect("a readable crates folder");
    let mut folders: Vec<PathBuf> = entries
        .map(|entry| entry.expect("a crates entry").path())
        .filter(|folder| folder.join("Cargo.toml").is_file())
        .collect();
    folders.sort();
    folders
        .iter()
        .flat_map(|folder| {
            let src = folder.join("src");
            let mut files = if src.is_dir() {
                walk(&src).production
            } else {
                Vec::new()
            };
            let build_script = folder.join("build.rs");
            if build_script.is_file() {
                files.push(build_script);
            }
            files
        })
        .collect()
}

/// The production sources of every crate in the workspace, where an alias may be made.
pub fn workspace_sources() -> Vec<String> {
    workspace_production()
        .into_iter()
        .map(|path| std::fs::read_to_string(path).expect("a readable source"))
        .collect()
}

/// How often `item`, when it is the free function `function`, names `name`.
pub fn named_in_free_fn(item: &Item, function: &str, name: &str) -> usize {
    let Item::Fn(free) = item else {
        return 0;
    };
    if free.sig.ident != function {
        return 0;
    }
    identifiers(free.to_token_stream())
        .iter()
        .filter(|ident| *ident == name)
        .count()
}

/// A short name for `item` in a finding: its kind and name, or an impl's type.
pub fn item_name(item: &Item) -> String {
    let named = |kind: &str, ident: &syn::Ident| format!("{kind} {ident}");
    match item {
        Item::Fn(function) => named("fn", &function.sig.ident),
        Item::Struct(structure) => named("struct", &structure.ident),
        Item::Enum(enumeration) => named("enum", &enumeration.ident),
        Item::Trait(tr) => named("trait", &tr.ident),
        Item::Mod(module) => named("mod", &module.ident),
        Item::Type(alias) => named("type", &alias.ident),
        Item::Const(constant) => named("const", &constant.ident),
        Item::Static(stat) => named("static", &stat.ident),
        Item::Impl(implementation) => {
            let self_type = implementation.self_ty.to_token_stream();
            format!("impl {self_type}")
        }
        other => other
            .to_token_stream()
            .to_string()
            .chars()
            .take(80)
            .collect(),
    }
}

/// A fresh source tree named `name` holding `files`, each at its path under `src`.
pub fn fixture_tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(env!("CARGO_CRATE_NAME"))
        .join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("the old fixture tree is removed");
    }
    let src = root.join("src");
    for (path, source) in files {
        let file = src.join(path);
        let folder = file.parent().expect("a fixture file sits in a folder");
        std::fs::create_dir_all(folder).expect("the fixture folder is made");
        std::fs::write(&file, source).expect("the fixture file is written");
    }
    src
}

/// A workspace crate as cargo metadata names it: its package name, its folder, and each of
/// its dependencies with its kind (`None` for a normal one, else `dev` or `build`).
#[derive(Debug, Clone)]
pub struct Member {
    pub name: String,
    pub folder: PathBuf,
    pub dependencies: Vec<(String, Option<String>)>,
}

impl Member {
    /// Whether this crate depends on `name` other than as a dev-dependency.
    pub fn depends_normally_on(&self, name: &str) -> bool {
        self.dependencies
            .iter()
            .any(|(dependency, kind)| dependency == name && kind.as_deref() != Some("dev"))
    }
}

/// The workspace's crates, as `cargo metadata` prints them.
pub fn metadata() -> serde_json::Value {
    let manifest = crates()
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .join("Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON")
}

/// The crates `metadata` lists, each with its folder and its dependencies.
pub fn members_of(metadata: &serde_json::Value) -> Vec<Member> {
    let packages = metadata["packages"].as_array().expect("a package list");
    packages
        .iter()
        .map(|package| {
            let manifest = package["manifest_path"].as_str().expect("a manifest path");
            let dependencies = package["dependencies"]
                .as_array()
                .expect("a dependency list")
                .iter()
                .map(|dependency| {
                    let name = dependency["name"].as_str().expect("a dependency name");
                    let kind = dependency["kind"].as_str().map(str::to_owned);
                    (name.to_owned(), kind)
                })
                .collect();
            Member {
                name: package["name"].as_str().expect("a package name").to_owned(),
                folder: Path::new(manifest)
                    .parent()
                    .expect("a manifest sits in a folder")
                    .to_path_buf(),
                dependencies,
            }
        })
        .collect()
}

/// The drivers among `members`: the composition root and every crate with a normal
/// dependency on it, the test rig aside.
pub fn drivers_of(members: &[Member]) -> Vec<Member> {
    members
        .iter()
        .filter(|member| member.name == COMPOSITION || member.depends_normally_on(COMPOSITION))
        .filter(|member| member.name != TEST_SUPPORT)
        .cloned()
        .collect()
}

/// The workspace's driver crates, by cargo metadata.
pub fn drivers() -> Vec<Member> {
    drivers_of(&members_of(&metadata()))
}

/// The crates the node-port scan reads: the drivers, and the use-case layer.
pub fn node_port_scanned() -> Vec<Member> {
    let members = members_of(&metadata());
    let mut scanned = drivers_of(&members);
    let application = members
        .iter()
        .find(|member| member.name == APPLICATION)
        .expect("the use-case layer is a member");
    scanned.push(application.clone());
    scanned
}
