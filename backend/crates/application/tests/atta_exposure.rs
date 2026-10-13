//! Nothing in `application` hands a driver one of Atta's types: no `pub use` of Atta, and no
//! `pub` item whose signature names Atta, either as `atta::…` or through a name the file
//! imported from Atta (directly or through `crate::den`'s re-exports). `pub(crate)` is fine.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use quote::ToTokens;
use syn::{Fields, ImplItem, Item, UseTree, Visibility};

/// The engine's crate name.
const ENGINE: &str = "atta";

/// One leaf of a `use` tree: the path that leads to it and the name it binds.
struct Leaf {
    path: Vec<String>,
    original: String,
    bound: String,
}

/// Every non-glob leaf of `tree`, with `prefix` the path above it.
fn leaves(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<Leaf>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            leaves(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let leaf = Leaf {
                path: prefix.clone(),
                original: name.ident.to_string(),
                bound: name.ident.to_string(),
            };
            out.push(leaf);
        }
        UseTree::Rename(rename) => {
            let leaf = Leaf {
                path: prefix.clone(),
                original: rename.ident.to_string(),
                bound: rename.rename.to_string(),
            };
            out.push(leaf);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                leaves(item, prefix, out);
            }
        }
        UseTree::Glob(_) => {}
    }
}

/// Every `use` leaf in `items`, nested inline modules included.
fn use_leaves(items: &[Item]) -> Vec<Leaf> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Use(item_use) => leaves(&item_use.tree, &mut Vec::new(), &mut out),
            Item::Mod(module) => {
                if let Some((_, content)) = &module.content {
                    out.extend(use_leaves(content));
                }
            }
            _ => {}
        }
    }
    out
}

/// Whether a leaf names something of Atta's: imported from `atta` itself, or one of
/// `crate::den`'s re-exports of Atta's names.
fn is_engine_leaf(leaf: &Leaf, den_reexports: &BTreeSet<String>) -> bool {
    let from_engine = leaf.path.first().is_some_and(|first| first == ENGINE);
    let through_den = leaf.path == ["crate", "den"] && den_reexports.contains(&leaf.original);
    from_engine || through_den
}

/// Whether the tokens of `node` name Atta or one of `names`.
fn names_engine(node: &impl ToTokens, names: &BTreeSet<String>) -> bool {
    let tokens = node.to_token_stream().to_string();
    tokens
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|word| word == ENGINE || names.contains(word))
}

fn is_pub(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

/// The types of a struct's or variant's fields that `all` or their own `pub` exposes.
fn exposed_field_types(fields: &Fields, all: bool) -> Vec<&syn::Type> {
    fields
        .iter()
        .filter(|field| all || is_pub(&field.vis))
        .map(|field| &field.ty)
        .collect()
}

/// The public items among `items` that name Atta, nested inline modules included.
fn exposing_items(items: &[Item], names: &BTreeSet<String>, out: &mut Vec<String>) {
    for item in items {
        let exposed = match item {
            Item::Use(item_use) => is_pub(&item_use.vis) && names_engine(&item_use.tree, names),
            Item::Fn(function) => is_pub(&function.vis) && names_engine(&function.sig, names),
            Item::Struct(structure) => {
                is_pub(&structure.vis)
                    && exposed_field_types(&structure.fields, false)
                        .iter()
                        .any(|ty| names_engine(*ty, names))
            }
            Item::Enum(enumeration) => {
                is_pub(&enumeration.vis)
                    && enumeration.variants.iter().any(|variant| {
                        exposed_field_types(&variant.fields, true)
                            .iter()
                            .any(|ty| names_engine(*ty, names))
                    })
            }
            Item::Type(alias) => is_pub(&alias.vis) && names_engine(&alias.ty, names),
            Item::Const(constant) => is_pub(&constant.vis) && names_engine(&constant.ty, names),
            Item::Static(stat) => is_pub(&stat.vis) && names_engine(&stat.ty, names),
            Item::Trait(tr) => is_pub(&tr.vis) && names_engine(tr, names),
            Item::Impl(implementation) => impl_exposes(implementation, names),
            Item::Mod(module) => {
                if let Some((_, content)) = &module.content {
                    exposing_items(content, names, out);
                }
                false
            }
            _ => false,
        };
        if exposed {
            let line = item.to_token_stream().to_string();
            out.push(line.chars().take(160).collect());
        }
    }
}

/// Whether an impl block exposes Atta: a `pub` inherent method or constant naming it, or a
/// trait impl (of a trait not Atta's own) whose header names it.
fn impl_exposes(implementation: &syn::ItemImpl, names: &BTreeSet<String>) -> bool {
    match &implementation.trait_ {
        None => implementation.items.iter().any(|item| match item {
            ImplItem::Fn(method) => is_pub(&method.vis) && names_engine(&method.sig, names),
            ImplItem::Const(constant) => is_pub(&constant.vis) && names_engine(&constant.ty, names),
            _ => false,
        }),
        Some((_, trait_path, _)) => {
            let first = trait_path
                .segments
                .first()
                .map(|segment| segment.ident.to_string());
            let last = trait_path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            let engine_trait =
                first.as_deref() == Some(ENGINE) || last.is_some_and(|name| names.contains(&name));
            !engine_trait
                && (names_engine(trait_path, names)
                    || names_engine(&*implementation.self_ty, names))
        }
    }
}

/// The public items of one source file that name Atta.
fn exposures(source: &str, den_reexports: &BTreeSet<String>) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let names: BTreeSet<String> = use_leaves(&file.items)
        .into_iter()
        .filter(|leaf| is_engine_leaf(leaf, den_reexports))
        .map(|leaf| leaf.bound)
        .collect();
    let mut out = Vec::new();
    exposing_items(&file.items, &names, &mut out);
    out
}

/// Every Rust source of `directory`, recursively, unit-test files left out.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(directory).expect("a readable directory");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let is_test_file = path
            .file_name()
            .is_some_and(|name| name == "tests.rs" || name == "proptests.rs");
        if path.is_dir() {
            files.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") && !is_test_file {
            files.push(path);
        }
    }
    files
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The names `crate::den` re-exports from Atta.
fn den_reexports() -> BTreeSet<String> {
    let source = std::fs::read_to_string(src().join("den.rs")).expect("den.rs reads");
    let file = syn::parse_file(&source).expect("den.rs parses");
    use_leaves(&file.items)
        .into_iter()
        .filter(|leaf| leaf.path.first().is_some_and(|first| first == ENGINE))
        .map(|leaf| leaf.bound)
        .collect()
}

#[test]
fn den_re_exports_atta_names_only_inside_the_crate() {
    let reexports = den_reexports();

    assert!(reexports.contains("Segment"), "{reexports:?}");
}

#[test]
fn no_public_item_names_an_atta_type() {
    let reexports = den_reexports();

    let offenders: Vec<String> = sources(&src())
        .into_iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            exposures(&source, &reexports)
                .into_iter()
                .map(move |item| format!("{}: {item}", path.display()))
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "public items name Atta:\n{offenders:#?}"
    );
}

#[test]
fn the_guard_catches_each_kind_of_exposure() {
    let reexports = BTreeSet::from(["Segment".to_owned()]);
    let planted = [
        "pub use atta::Segment;",
        "pub use atta::{Caps, Segment as Piece};",
        "pub fn leak() -> atta::Segment { todo!() }",
        "use atta::Segment; pub fn leak(segment: Segment) {}",
        "use crate::den::Segment; pub struct Leak { pub segment: Segment }",
        "use atta::Level; pub enum Leak { Level(Level) }",
        "pub type Leak = atta::RealPath;",
        "pub struct Wrapper; impl Wrapper { pub fn leak(&self) -> atta::Caps { todo!() } }",
        "pub struct Wrapper; impl From<atta::Caps> for Wrapper { fn from(_: atta::Caps) -> Self { Wrapper } }",
        "pub mod inner { pub fn leak() -> atta::Segment { todo!() } }",
    ];

    for source in planted {
        assert!(!exposures(source, &reexports).is_empty(), "{source}");
    }
}

#[test]
fn the_guard_lets_crate_private_uses_pass() {
    let reexports = BTreeSet::from(["Segment".to_owned()]);
    let allowed = [
        "pub(crate) use atta::Segment;",
        "use atta::Segment; pub(crate) fn inside(segment: Segment) {}",
        "pub struct Wrapper(atta::PageToken);",
        "use atta::Segment; pub struct Wrapper { segment: Segment }",
        "use atta::Host; pub struct DenHost; impl Host for DenHost {}",
        "pub struct Wrapper; impl Wrapper { pub(crate) fn inner(&self) -> atta::Caps { todo!() } }",
    ];

    for source in allowed {
        assert_eq!(
            exposures(source, &reexports),
            Vec::<String>::new(),
            "{source}"
        );
    }
}
