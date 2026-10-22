//! CI guard: no actor port takes or returns a `NodeId`.
//!
//! Actors are addressed by DID; a node id is internal to the Den. The guard reads every
//! port of `identity` and every port of `kernel` but the node port itself, and refuses a
//! function signature naming `NodeId`, directly or through a renamed import or a type alias
//! made anywhere in either crate. It reads the sources with `syn`: an alias compiles fine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{ImplItemType, ItemType, TraitItemType, UseRename};

/// The node key no actor port may carry.
const NODE_ID: &str = "NodeId";

/// The names of unit-test and property-test files, which hold no port.
const TEST_FILES: [&str; 2] = ["tests.rs", "proptests.rs"];

/// Every identifier in `tokens`, groups and macro bodies included, raw ones without `r#`;
/// a lifetime's name is not one.
fn identifiers(tokens: TokenStream) -> Vec<String> {
    let mut out = Vec::new();
    let mut after_quote = false;
    for tree in tokens {
        match &tree {
            TokenTree::Group(group) => out.extend(identifiers(group.stream())),
            TokenTree::Ident(ident) if !after_quote => out.push(ident.unraw().to_string()),
            TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
        after_quote = matches!(&tree, TokenTree::Punct(punct) if punct.as_char() == '\'');
    }
    out
}

fn names_forbidden(tokens: TokenStream, forbidden: &BTreeSet<String>) -> bool {
    identifiers(tokens)
        .iter()
        .any(|ident| forbidden.contains(ident))
}

/// Collects every name a file binds to a forbidden one: `use … X as Y` (never `_`) and
/// `type Y = …X…`, at any depth.
struct Aliases<'a> {
    forbidden: &'a BTreeSet<String>,
    found: BTreeSet<String>,
}

impl Aliases<'_> {
    fn alias(&mut self, bound: &syn::Ident, ty: &syn::Type) {
        if names_forbidden(ty.to_token_stream(), self.forbidden) {
            self.found.insert(bound.unraw().to_string());
        }
    }
}

impl<'ast> Visit<'ast> for Aliases<'_> {
    fn visit_use_rename(&mut self, rename: &'ast UseRename) {
        let original = rename.ident.unraw().to_string();
        let bound = rename.rename.unraw().to_string();
        if self.forbidden.contains(&original) && bound != "_" {
            self.found.insert(bound);
        }
    }

    fn visit_item_type(&mut self, alias: &'ast ItemType) {
        self.alias(&alias.ident, &alias.ty);
        visit::visit_item_type(self, alias);
    }

    fn visit_impl_item_type(&mut self, alias: &'ast ImplItemType) {
        self.alias(&alias.ident, &alias.ty);
        visit::visit_impl_item_type(self, alias);
    }

    fn visit_trait_item_type(&mut self, alias: &'ast TraitItemType) {
        if let Some((_, ty)) = &alias.default {
            self.alias(&alias.ident, ty);
        }
        visit::visit_trait_item_type(self, alias);
    }
}

/// `NodeId` and every name `files` bind to it, through any chain of aliases.
fn forbidden_names(files: &[syn::File]) -> BTreeSet<String> {
    let mut forbidden = BTreeSet::from([NODE_ID.to_owned()]);
    loop {
        let mut aliases = Aliases {
            forbidden: &forbidden,
            found: forbidden.clone(),
        };
        for file in files {
            aliases.visit_file(file);
        }
        let grown = aliases.found;
        if grown == forbidden {
            return forbidden;
        }
        forbidden = grown;
    }
}

/// Whether `tree` ends a function signature: its `;` or its body.
fn ends_signature(tree: &TokenTree) -> bool {
    match tree {
        TokenTree::Punct(punct) => punct.as_char() == ';',
        TokenTree::Group(group) => group.delimiter() == Delimiter::Brace,
        TokenTree::Ident(_) | TokenTree::Literal(_) => false,
    }
}

/// The name of each function in `tokens` whose signature names a forbidden name, groups and
/// macro bodies included.
fn signatures_naming(tokens: TokenStream, forbidden: &BTreeSet<String>) -> Vec<String> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out = Vec::new();
    for (index, tree) in trees.iter().enumerate() {
        if let TokenTree::Group(group) = tree {
            out.extend(signatures_naming(group.stream(), forbidden));
            continue;
        }
        let (TokenTree::Ident(keyword), Some(TokenTree::Ident(name))) =
            (tree, trees.get(index + 1))
        else {
            continue;
        };
        if keyword != "fn" {
            continue;
        }
        let signature: TokenStream = trees[index..]
            .iter()
            .take_while(|tree| !ends_signature(tree))
            .cloned()
            .collect();
        if names_forbidden(signature, forbidden) {
            out.push(name.unraw().to_string());
        }
    }
    out
}

fn parse(file: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("guard cannot parse {file}: {error}"))
}

/// The `fn` of each port in `ports` whose signature names a node id, as `<file>: fn <name>`;
/// aliases come from `ports` and `elsewhere` both.
fn offenders(ports: &[(String, String)], elsewhere: &[String]) -> Vec<String> {
    let port_files: Vec<(String, syn::File)> = ports
        .iter()
        .map(|(file, source)| (file.clone(), parse(file, source)))
        .collect();
    let all_files: Vec<syn::File> = port_files
        .iter()
        .map(|(_, parsed)| parsed.clone())
        .chain(
            elsewhere
                .iter()
                .map(|source| parse("a source of identity or kernel", source)),
        )
        .collect();
    let forbidden = forbidden_names(&all_files);
    port_files
        .iter()
        .flat_map(|(file, parsed)| {
            signatures_naming(parsed.to_token_stream(), &forbidden)
                .into_iter()
                .map(move |name| format!("{file}: fn {name}"))
        })
        .collect()
}

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .to_path_buf()
}

/// Every `*.rs` file under `dir`, recursively, test files left out.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("guard cannot read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        let is_test_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| TEST_FILES.contains(&name));
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") && !is_test_file {
            out.push(path);
        }
    }
    out
}

/// The port files of `krate`: `src/ports.rs` and every file under `src/ports/`.
fn port_files(krate: &str) -> Vec<PathBuf> {
    let src = crates().join(krate).join("src");
    let facade = src.join("ports.rs");
    assert!(facade.is_file(), "guard expects `{krate}/src/ports.rs`");
    let mut files = rust_files(&src.join("ports"));
    files.push(facade);
    files
}

/// The node port's own files, which the guard leaves out.
fn node_port_files() -> Vec<PathBuf> {
    let ports = crates().join("kernel/src/ports");
    let mut files = vec![ports.join("node.rs")];
    let leaves = ports.join("node");
    if leaves.is_dir() {
        files.extend(rust_files(&leaves));
    }
    files
}

/// The actor ports: every port file of `identity` and `kernel`, the node port's left out.
fn actor_port_files() -> Vec<PathBuf> {
    let left_out = node_port_files();
    port_files("identity")
        .into_iter()
        .chain(port_files("kernel"))
        .filter(|path| !left_out.contains(path))
        .collect()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("guard cannot read {}: {error}", path.display()))
}

#[test]
fn the_guard_reads_identity_s_ports_and_kernel_s_but_the_node_port() {
    let files = actor_port_files();

    let user_port = crates().join("identity/src/ports/user.rs");
    let node_port = crates().join("kernel/src/ports/node.rs");
    assert!(files.contains(&user_port), "{files:#?}");
    assert!(node_port.is_file());
    assert!(!files.contains(&node_port), "{files:#?}");
}

#[test]
fn no_actor_port_takes_or_returns_a_node_id() {
    let ports: Vec<(String, String)> = actor_port_files()
        .iter()
        .map(|path| (path.display().to_string(), read(path)))
        .collect();
    let elsewhere: Vec<String> = ["kernel", "identity"]
        .iter()
        .flat_map(|krate| rust_files(&crates().join(krate).join("src")))
        .map(|path| read(&path))
        .collect();

    let found = offenders(&ports, &elsewhere);

    assert!(
        found.is_empty(),
        "actor ports take or return a node id — an actor is addressed by DID:\n  {}",
        found.join("\n  ")
    );
}

fn port(source: &str) -> (String, String) {
    ("port.rs".to_owned(), source.to_owned())
}

#[test]
fn the_guard_refuses_a_node_id_however_it_is_named() {
    let planted = [
        (
            "find",
            "pub trait UserStore { async fn find(&self, id: &NodeId) -> anyhow::Result<()>; }",
        ),
        (
            "find",
            "pub trait UserStore { fn find(&self) -> anyhow::Result<Option<kernel::elements::node::NodeId>>; }",
        ),
        (
            "find",
            "pub trait UserStore { fn find(&self, id: r#NodeId); }",
        ),
        (
            "find",
            "use kernel::elements::node::NodeId as Address; pub trait UserStore { fn find(&self, id: &Address); }",
        ),
        (
            "find",
            "use crate::elements::node::{NodeRow, NodeId as Address}; pub trait UserStore { fn find(&self, id: Address); }",
        ),
        (
            "find",
            "pub type Address = NodeId; pub trait UserStore { fn find(&self) -> Address; }",
        ),
        (
            "all",
            "type Address = NodeId; type Many = Vec<Address>; pub trait UserStore { fn all(&self) -> Many; }",
        ),
        (
            "each",
            "pub trait UserStore { fn each(&self, visit: &dyn Fn(NodeId)); }",
        ),
    ];

    for (name, source) in planted {
        let found = offenders(&[port(source)], &[]);

        let expected = [format!("port.rs: fn {name}")];
        assert_eq!(found, expected, "{source}");
    }
}

#[test]
fn an_alias_made_in_another_file_is_seen() {
    let elsewhere = ["pub type Address = super::node::NodeId;".to_owned()];
    let planted = port("pub trait UserStore { fn find(&self, id: &Address); }");

    let found = offenders(&[planted], &elsewhere);

    assert_eq!(found, ["port.rs: fn find"]);
}

#[test]
fn the_guard_passes_what_names_no_node_id() {
    let allowed = [
        "pub trait UserStore { fn find(&self, did: &Did) -> Option<UserRow>; }",
        "/// Never a NodeId.\npub trait UserStore { fn find(&self, did: &Did); }",
        "pub trait UserStore { fn find(&self, did: &Did); } // NodeId",
        "pub trait UserStore { /* NodeId */ fn find(&self, did: &Did); }",
        "pub trait UserStore { fn kind(&self) -> &'static str { \"NodeId\" } }",
        "pub trait UserStore { fn first(&self) -> char { 'N' } fn find(&self, did: &Did); }",
        "use crate::elements::node::NodeId as _; pub trait UserStore { fn find(&self, id: &_); }",
        "pub trait UserStore { fn find(&self, id: &NodeIdentity); }",
        "pub trait UserStore { fn find<'NodeId>(&self, did: &'NodeId Did); }",
    ];

    for source in allowed {
        let found = offenders(&[port(source)], &[]);
        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}
