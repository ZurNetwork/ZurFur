//! No driver and no use case reaches the Den's node port, and Atta's checked-name constructor
//! is named only in `DenHost::found`. The scan reads the production sources of `application`,
//! `api`, `cli` and `composition` token by token, macro bodies included, and matches every
//! identifier, so a call, a function reference, a qualified path and an alias all count.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use syn::{ImplItem, Item, UseTree, ext::IdentExt};

/// The identifiers that reach the node port: a unit of work's accessor, the port's traits and
/// the PostgreSQL adapter's types.
const NODE_PORT: [&str; 7] = [
    "nodes",
    "NodeRepo",
    "NodeStore",
    "NodeReads",
    "NodeWrites",
    "PgNodeStore",
    "PgNodeWrites",
];

/// Atta's checked-name constructor, named only in [`FOUND`] on [`DEN_HOST`] in the host file.
const HOST_CHECKED: &str = "host_checked";

/// The one method that may name [`HOST_CHECKED`].
const FOUND: &str = "found";

/// The type whose inherent [`FOUND`] may name [`HOST_CHECKED`].
const DEN_HOST: &str = "DenHost";

/// The crates whose production sources the scan reads.
const SCANNED_CRATES: [&str; 4] = ["application", "api", "cli", "composition"];

/// The names of unit-test and property-test files, skipped when declared under `#[cfg(test)]`.
const TEST_FILES: [&str; 2] = ["tests.rs", "proptests.rs"];

/// The folder holding every crate of the workspace.
fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .to_path_buf()
}

/// The one file whose [`FOUND`] may name [`HOST_CHECKED`].
fn host_file() -> PathBuf {
    crates().join("application/src/den/host.rs")
}

/// Every identifier in `tokens`, groups and macro bodies included, raw ones without `r#`.
fn identifiers(tokens: TokenStream) -> Vec<String> {
    tokens
        .into_iter()
        .flat_map(|tree| match tree {
            TokenTree::Group(group) => identifiers(group.stream()),
            TokenTree::Ident(ident) => vec![ident.unraw().to_string()],
            TokenTree::Punct(_) | TokenTree::Literal(_) => Vec::new(),
        })
        .collect()
}

fn is_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| TEST_FILES.contains(&name))
}

/// Whether `items` declare the out-of-line module `name` under `#[cfg(test)]`, at its own path.
fn declares_under_cfg_test(items: &[Item], name: &str) -> bool {
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

/// Whether the module that owns test file `path` declares it under `#[cfg(test)]`: `lib.rs`
/// or `main.rs` for a file at `src`'s top, else the folder's `<folder>.rs` or `mod.rs`.
fn declared_under_cfg_test(path: &Path, src: &Path) -> bool {
    let folder = path.parent().expect("a test file sits in a folder");
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("a test file's name");
    let parents = if folder == src {
        vec![src.join("lib.rs"), src.join("main.rs")]
    } else {
        vec![folder.with_extension("rs"), folder.join("mod.rs")]
    };
    parents
        .iter()
        .filter(|parent| parent.is_file())
        .any(|parent| {
            let source = std::fs::read_to_string(parent).expect("a readable parent module");
            let file = syn::parse_file(&source).expect("the parent module parses");
            declares_under_cfg_test(&file.items, name)
        })
}

/// One crate's sources: the production files the scan reads, and each test file it would
/// skip though no parent module declares it under `#[cfg(test)]`.
#[derive(Default)]
struct Walk {
    production: Vec<PathBuf>,
    undeclared: Vec<PathBuf>,
}

fn walk_folder(folder: &Path, src: &Path, walk: &mut Walk) {
    let entries = std::fs::read_dir(folder).expect("a readable folder");
    for entry in entries {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            walk_folder(&path, src, walk);
        } else if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        } else if !is_test_file(&path) {
            walk.production.push(path);
        } else if !declared_under_cfg_test(&path, src) {
            walk.undeclared.push(path);
        }
    }
}

/// Every Rust source under `src`, test files declared under `#[cfg(test)]` skipped.
fn walk(src: &Path) -> Walk {
    let mut walk = Walk::default();
    walk_folder(src, src, &mut walk);
    walk
}

/// Every name `items` bind to a forbidden one: a renamed import of it, or a type alias naming
/// it. A rename to `_` binds nothing.
fn aliases_in(items: &[Item], forbidden: &BTreeSet<String>, out: &mut BTreeSet<String>) {
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

/// The forbidden identifiers: the node port's, [`HOST_CHECKED`], and every name `sources` bind
/// to one of them, through any chain of aliases.
fn forbidden_names(sources: &[String]) -> BTreeSet<String> {
    let files: Vec<syn::File> = sources
        .iter()
        .map(|source| syn::parse_file(source).expect("the source parses"))
        .collect();
    let mut forbidden: BTreeSet<String> = NODE_PORT
        .iter()
        .chain([&HOST_CHECKED])
        .map(|name| (*name).to_owned())
        .collect();
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

/// The production sources of every crate in the workspace, where an alias may be made.
fn workspace_sources() -> Vec<String> {
    let entries = std::fs::read_dir(crates()).expect("a readable crates folder");
    entries
        .map(|entry| entry.expect("a crates entry").path().join("src"))
        .filter(|src| src.is_dir())
        .flat_map(|src| walk(&src).production)
        .map(|path| std::fs::read_to_string(path).expect("a readable source"))
        .collect()
}

/// How often `item`, an inherent impl of [`DEN_HOST`], names [`HOST_CHECKED`] in its [`FOUND`].
fn named_in_found(item: &Item) -> usize {
    let Item::Impl(implementation) = item else {
        return 0;
    };
    let syn::Type::Path(self_type) = &*implementation.self_ty else {
        return 0;
    };
    let on_den_host = self_type
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == DEN_HOST);
    if !on_den_host || implementation.trait_.is_some() {
        return 0;
    }
    implementation
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if method.sig.ident == FOUND => Some(method),
            _ => None,
        })
        .flat_map(|method| identifiers(method.to_token_stream()))
        .filter(|ident| ident == HOST_CHECKED)
        .count()
}

/// A short name for `item` in a finding: its kind and name, or an impl's type.
fn item_name(item: &Item) -> String {
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

/// Each forbidden identifier in `source`, with the item it sits in; in the host file, the
/// [`HOST_CHECKED`] inside `DenHost::found` passes.
fn findings(source: &str, forbidden: &BTreeSet<String>, is_host_file: bool) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let inner_attributes: TokenStream = file
        .attrs
        .iter()
        .map(|attribute| attribute.to_token_stream())
        .collect();
    let mut out: Vec<String> = identifiers(inner_attributes)
        .into_iter()
        .filter(|ident| forbidden.contains(ident))
        .map(|ident| format!("`{ident}` in the file's attributes"))
        .collect();
    for item in &file.items {
        let mut allowed = if is_host_file {
            named_in_found(item)
        } else {
            0
        };
        let summary = item_name(item);
        for ident in identifiers(item.to_token_stream()) {
            if !forbidden.contains(&ident) {
                continue;
            }
            if ident == HOST_CHECKED && allowed > 0 {
                allowed -= 1;
                continue;
            }
            out.push(format!("`{ident}` in {summary}"));
        }
    }
    out
}

/// Each forbidden identifier in the production sources under `src`.
fn tree_findings(src: &Path, forbidden: &BTreeSet<String>) -> Vec<String> {
    let host = host_file();
    walk(src)
        .production
        .into_iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            findings(&source, forbidden, path == host)
                .into_iter()
                .map(move |finding| format!("{}: {finding}", path.display()))
        })
        .collect()
}

fn scanned_srcs() -> Vec<PathBuf> {
    SCANNED_CRATES
        .iter()
        .map(|name| crates().join(name).join("src"))
        .collect()
}

#[test]
fn the_scan_reads_every_scanned_crate_and_the_host_file() {
    for src in scanned_srcs() {
        let production = walk(&src).production;

        assert!(!production.is_empty(), "{} has no sources", src.display());
    }
    let application = walk(&crates().join("application/src")).production;
    assert!(application.contains(&host_file()));
}

#[test]
fn every_skipped_test_file_is_declared_under_cfg_test() {
    let undeclared: Vec<PathBuf> = scanned_srcs()
        .iter()
        .flat_map(|src| walk(src).undeclared)
        .collect();

    assert!(
        undeclared.is_empty(),
        "test files no parent module declares under #[cfg(test)]:\n{undeclared:#?}"
    );
}

#[test]
fn no_production_source_reaches_the_node_port_or_the_checked_name() {
    let forbidden = forbidden_names(&workspace_sources());

    let offenders: Vec<String> = scanned_srcs()
        .iter()
        .flat_map(|src| tree_findings(src, &forbidden))
        .collect();

    assert!(
        offenders.is_empty(),
        "production sources reach the node port or name host_checked:\n{offenders:#?}"
    );
}

#[test]
fn found_names_the_checked_name_once() {
    let source = std::fs::read_to_string(host_file()).expect("host.rs reads");
    let file = syn::parse_file(&source).expect("host.rs parses");

    let named: usize = file.items.iter().map(named_in_found).sum();

    assert_eq!(named, 1);
}

fn plain_forbidden() -> BTreeSet<String> {
    forbidden_names(&[])
}

#[test]
fn the_scan_refuses_each_way_of_reaching_the_node_port() {
    let forbidden = plain_forbidden();
    let planted = [
        "fn reach(uow: Unit<'_>) { let repo = uow.nodes(); }",
        "fn reach(uow: &mut dyn UnitOfWork) { let repo = UnitOfWork::nodes(&mut *uow); }",
        "fn reach() { let vend = UnitOfWork::nodes; }",
        "fn reach(units: Vec<Box<dyn UnitOfWork>>) { units.into_iter().map(<dyn UnitOfWork>::nodes); }",
        "fn reach(repo: Box<dyn domain::ports::NodeRepo>) {}",
        "use domain::ports::{NodeReads, NodeWrites};",
        "use kernel::ports::NodeStore;",
        "fn reach(pool: PgPool) { let store = adapter_pg::PgNodeStore::new(pool); }",
        "fn reach(writes: adapter_pg::PgNodeWrites<'_>) {}",
        "use adapter_pg::PgNodeStore as Store;",
        "fn reach(uow: Unit<'_>) { tokio::join!(async { uow.nodes() }); }",
        "macro_rules! reach { ($unit:expr) => { $unit.nodes() }; }",
        "fn reach(uow: Unit<'_>) { uow.r#nodes(); }",
        "struct Wrapper(Box<dyn UnitOfWork>); impl UnitOfWork for Wrapper { fn nodes(&mut self) -> Box<dyn Vended + '_> { self.0.vend() } }",
        "#[cfg(test)] mod tests { fn reach(uow: Unit<'_>) { uow.nodes(); } }",
        "fn reach() { let nodes = Vec::<u8>::new(); }",
    ];

    for source in planted {
        assert!(!findings(source, &forbidden, false).is_empty(), "{source}");
        assert!(!findings(source, &forbidden, true).is_empty(), "{source}");
    }
}

#[test]
fn the_scan_refuses_the_checked_name_outside_found() {
    let forbidden = plain_forbidden();
    let elsewhere = [
        "fn name(text: String) -> DisplayName { DisplayName::host_checked(text) }",
        "fn names(texts: Vec<String>) { texts.into_iter().map(atta::DisplayName::host_checked); }",
        "use atta::DisplayName as Shown; fn name(text: String) -> Shown { Shown::host_checked(text) }",
        "fn names(text: String) { let names = vec![atta::DisplayName::host_checked(text)]; }",
        "impl<'a> DenHost<'a> { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
    ];
    let in_the_host_file = [
        "fn found(text: String) -> DisplayName { DisplayName::host_checked(text) }",
        "impl<'a> DenHost<'a> { fn mount(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
        "impl Other { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
        "impl Host for DenHost<'_> { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
        "impl<'a> DenHost<'a> { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } fn mount(&self, text: String) { DisplayName::host_checked(text); } }",
        "mod inner { impl<'a> DenHost<'a> { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } } }",
    ];

    for source in elsewhere {
        assert!(!findings(source, &forbidden, false).is_empty(), "{source}");
    }
    for source in in_the_host_file {
        assert!(!findings(source, &forbidden, true).is_empty(), "{source}");
    }
}

#[test]
fn found_may_name_the_checked_name_but_not_the_node_port() {
    let forbidden = plain_forbidden();
    let allowed = "impl<'a> DenHost<'a> { fn found(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }";
    let reaching = "impl<'a> DenHost<'a> { fn found(&self, uow: Unit<'_>, text: String) -> DisplayName { uow.nodes(); DisplayName::host_checked(text) } }";

    let allowed_findings = findings(allowed, &forbidden, true);
    let reaching_findings = findings(reaching, &forbidden, true);

    assert_eq!(allowed_findings, Vec::<String>::new());
    assert_eq!(reaching_findings.len(), 1, "{reaching_findings:#?}");
}

#[test]
fn words_that_are_not_identifiers_pass() {
    let forbidden = plain_forbidden();
    let allowed = [
        "/// Lists the soft-deleted nodes the viewer may open.\nfn list() {}",
        "//! The nodes below the root.\nfn list() {}",
        "fn list() { // the nodes\n}",
        "fn word() -> &'static str { \"nodes host_checked NodeRepo\" }",
        "fn list(child_nodes: Vec<NodeRow>, id: NodeId) {}",
    ];

    for source in allowed {
        let found = findings(source, &forbidden, false);
        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

#[test]
fn an_alias_made_in_another_crate_is_refused() {
    let elsewhere = [
        "pub use kernel::ports::NodeRepo as Repo;".to_owned(),
        "pub mod stores { pub type Store = adapter_pg::PgNodeStore; }".to_owned(),
        "pub type Chained = Store;".to_owned(),
        "pub use kernel::ports::{NodeStore as _, NodeWrites as Writes};".to_owned(),
    ];
    let forbidden = forbidden_names(&elsewhere);
    let planted = [
        "fn reach(repo: Box<dyn Repo>) {}",
        "fn reach(pool: PgPool) { let store = stores::Store::new(pool); }",
        "fn reach(pool: PgPool) { let store = Chained::new(pool); }",
        "fn reach(writes: Box<dyn Writes>) {}",
    ];

    assert!(!forbidden.contains("_"));
    for source in planted {
        assert!(!findings(source, &forbidden, false).is_empty(), "{source}");
    }
}

/// A fresh source tree named `name` holding `files`, each at its path under `src`.
fn fixture_tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("node_and_name_seams")
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

#[test]
fn a_source_in_any_folder_is_read() {
    let src = fixture_tree(
        "any_folder",
        &[
            ("lib.rs", "mod den;"),
            ("den.rs", "mod read;"),
            ("den/read.rs", "fn reach(uow: Unit<'_>) { uow.nodes(); }"),
        ],
    );

    let found = tree_findings(&src, &plain_forbidden());

    assert_eq!(found.len(), 1, "{found:#?}");
}

#[test]
fn a_test_file_is_skipped_only_when_declared_under_cfg_test() {
    let reaching = "fn reach(uow: Unit<'_>) { uow.nodes(); }";
    let src = fixture_tree(
        "declared",
        &[
            (
                "lib.rs",
                "mod rename;\nmod routes;\n#[cfg(test)]\nmod tests;",
            ),
            ("tests.rs", reaching),
            (
                "rename.rs",
                "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod proptests;",
            ),
            ("rename/tests.rs", reaching),
            ("rename/proptests.rs", reaching),
            ("routes/mod.rs", "#[cfg(test)]\nmod tests;"),
            ("routes/tests.rs", reaching),
        ],
    );

    let declared = walk(&src);

    assert_eq!(declared.undeclared, Vec::<PathBuf>::new());
    assert_eq!(declared.production.len(), 3, "{:#?}", declared.production);
    assert_eq!(
        tree_findings(&src, &plain_forbidden()),
        Vec::<String>::new()
    );
}

#[test]
fn a_test_file_no_parent_declares_under_cfg_test_is_refused() {
    let reaching = "fn reach(uow: Unit<'_>) { uow.nodes(); }";
    let parents = [
        ("undeclared", "mod other;"),
        ("without_cfg_test", "mod tests;"),
        ("under_another_cfg", "#[cfg(feature = \"x\")]\nmod tests;"),
        (
            "moved",
            "#[cfg(test)]\n#[path = \"elsewhere.rs\"]\nmod tests;",
        ),
        ("inline", "#[cfg(test)]\nmod tests {}"),
    ];

    for (name, parent) in parents {
        let src = fixture_tree(
            name,
            &[
                ("lib.rs", "mod rename;"),
                ("rename.rs", parent),
                ("rename/tests.rs", reaching),
            ],
        );

        let undeclared = walk(&src).undeclared;

        assert_eq!(undeclared, [src.join("rename/tests.rs")], "{name}");
    }
}
