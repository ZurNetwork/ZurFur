//! The den's write gate is the only way to write a commission's node or fields. These scans
//! hold its seams: the write capability's surface, its one definition and its one issuer across
//! the workspace and the issuer's callers in the den, the lent view's surface and its latch,
//! nothing else in the gate file reaching the lent view's fields, no compiled source the scans
//! can't read, the write host and the gate named once each, the write use case kept inside the
//! crate, nothing of it (and no raw SQL or in-memory seeder) in a driver, no lint silencer in
//! any production file of the use-case layer, every method of the commission write view taking
//! kernel's write capability in place of the commission's id or named, and none with a default
//! body, the named writes clippy holds exactly the ones `application`'s clippy settings ban,
//! the commission field writes not yet gated, frozen, and the commission tables written only by
//! the commission write view in each adapter, whatever port a write is called through: in
//! PostgreSQL, every statement writing a commission table in the commission's query folder, its
//! generated write functions named only in the write view, and the view built only by the unit;
//! in memory, the commission map changed only by the write view, the insert path, the lifecycle
//! seeder and the unit's staging and merge, its rows built only by the insert path, and the
//! view built only by the unit.

mod seams;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use seams::{
    NODE_PORT, crates, declares_under_cfg_test, drivers, drivers_of, fixture_tree, identifiers,
    item_name, members_of, metadata, under_cfg_test, walk, with_aliases, workspace_production,
    workspace_sources,
};
use syn::{ImplItem, Item, TraitItem, Type, UseTree, Visibility, ext::IdentExt};

/// The write capability's one constructor, named only at its definition and in [`ISSUE`].
const GATE_ISSUED: &str = "gate_issued";

/// The gate's one issuer function, the only caller of [`GATE_ISSUED`].
const ISSUE: &str = "issue";

/// The write capability's type.
const WRITE_CAPABILITY: &str = "WriteCapability";

/// The write host's type; its constructor is named once, in [`WRITTEN`].
const WRITE_HOST: &str = "DenWriteHost";

/// The den write's function that builds the write host.
const WRITTEN: &str = "written";

/// The gate function; it is named once, in the write host's `apply`.
const GATE: &str = "gate";

/// The write host's method that runs the gate.
const APPLY: &str = "DenWriteHost::apply";

/// The adapter's generated queries and the SQL toolkit: no driver names either.
const RAW_SQL: [&str; 2] = ["queries", "sqlx"];

/// The one driver file that may name `sqlx`, inside [`LEADER_LOCK`] only.
const SWEEP: &str = "api/src/sweep.rs";

/// The deadline sweeper's leader-lock function.
const LEADER_LOCK: &str = "sweep_pass_as_leader";

/// The one driver whose calls to a unit's commission repo and changelog clippy holds instead,
/// each with its own `#[expect]`: a token scan can't tell them from its use-case namespaces.
const API: &str = "api";

/// The unit of work's commission repo and changelog: no driver but [`API`] calls either.
const COMMISSION_REPOS: [&str; 2] = ["commissions", "changelog"];

/// The commission repo's field writes the gate doesn't hold yet, by name, each call site in
/// `application` frozen whether it has one today or not. Each leaves when its method takes a
/// write capability, until the list is empty.
const FIELD_WRITES: [&str; 7] = [
    "set_archived",
    "set_maturity",
    "set_direction_status",
    "set_deadline",
    "set_deadline_status",
    "declare_slots",
    "set_linked_channel",
];

/// The commission repo's writes counted only on a commission repo: their names are common.
/// Through any other receiver in `application`, a create needs the gate's capability, and
/// clippy's ban on `CommissionWrites::delete` holds a delete.
const REPO_WRITES: [&str; 2] = ["create", "delete"];

/// The SQL words a table name follows.
const SQL_KEYWORDS: [&str; 11] = [
    "FROM", "INTO", "UPDATE", "JOIN", "TABLE", "TRUNCATE", "COPY", "LOCK", "USING", "MERGE", "ON",
];

/// The opening of PostgreSQL's Unicode-escaped quoted identifier, compared without case: the
/// escapes can spell any name, the node table's included.
const UNICODE_IDENTIFIER: &str = "u&\"";

/// The den's node table.
const NODE_TABLE: &str = "node";

/// The ungated commission field writes in `application`, by file, function and method: each
/// leaves this list when the den's write gate takes it over, until the list is empty.
const FROZEN_FIELD_WRITES: [(&str, &str, &str); 9] = [
    (
        "commission/archive.rs",
        "Commissions::archive",
        "set_archived",
    ),
    (
        "commission/deadline/clear.rs",
        "Deadline::clear",
        "set_deadline",
    ),
    (
        "commission/deadline/set.rs",
        "Deadline::set",
        "set_deadline",
    ),
    (
        "commission/deadline/status.rs",
        "apply",
        "set_deadline_status",
    ),
    ("commission/delete.rs", "Commissions::delete", "delete"),
    (
        "commission/maturity/set.rs",
        "Maturity::run",
        "set_maturity",
    ),
    (
        "commission/slots/declare.rs",
        "Slots::declare",
        "declare_slots",
    ),
    (
        "commission/status/direction/set.rs",
        "Direction::set",
        "set_direction_status",
    ),
    (
        "commission/unarchive.rs",
        "Commissions::unarchive",
        "set_archived",
    ),
];

/// The file that defines the write capability.
fn capability_file() -> PathBuf {
    crates().join("kernel/src/ports/capability.rs")
}

/// The use-case layer's sources.
fn application_src() -> PathBuf {
    crates().join("application/src")
}

/// `path` relative to `src`, with `/` between its parts.
fn relative(path: &Path, src: &Path) -> String {
    let inside = path.strip_prefix(src).unwrap_or(path);
    inside
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}

fn parse(path: &Path) -> syn::File {
    let source = std::fs::read_to_string(path).expect("a readable source");
    syn::parse_file(&source).unwrap_or_else(|error| panic!("{} parses: {error}", path.display()))
}

/// The last name of a type: `DenWriteHost` for `DenWriteHost<'_>`, or the type as written.
fn type_name(typ: &Type) -> String {
    if let Type::Path(path) = typ
        && let Some(last) = path.path.segments.last()
    {
        return last.ident.to_string();
    }
    typ.to_token_stream().to_string()
}

/// A piece of a file a finding names: the function it sits in (`Type::method` in an impl or a
/// trait, `module::` before it in an inline module), or the item that holds no function. A
/// piece of an impl carries its self type, and the last name of its trait.
struct Piece {
    context: String,
    impl_of: Option<String>,
    trait_of: Option<String>,
    tokens: TokenStream,
}

fn pieces_in(items: &[Item], prefix: &str, out: &mut Vec<Piece>) {
    let piece = |context: String, impl_of: Option<String>, tokens: TokenStream| Piece {
        context,
        impl_of,
        trait_of: None,
        tokens,
    };
    for item in items {
        match item {
            Item::Fn(function) => out.push(piece(
                format!("{prefix}{}", function.sig.ident),
                None,
                function.to_token_stream(),
            )),
            Item::Impl(implementation) => {
                let self_name = type_name(&implementation.self_ty);
                let mut header = implementation.self_ty.to_token_stream();
                if let Some((_, trait_path, _)) = &implementation.trait_ {
                    header.extend(trait_path.to_token_stream());
                }
                header.extend(implementation.generics.to_token_stream());
                for attribute in &implementation.attrs {
                    header.extend(attribute.to_token_stream());
                }
                let trait_of = implementation
                    .trait_
                    .as_ref()
                    .and_then(|(_, trait_path, _)| trait_path.segments.last())
                    .map(|segment| segment.ident.unraw().to_string());
                let in_impl = |context: String, tokens: TokenStream| Piece {
                    context,
                    impl_of: Some(self_name.clone()),
                    trait_of: trait_of.clone(),
                    tokens,
                };
                out.push(in_impl(format!("{prefix}impl {self_name}"), header));
                for impl_item in &implementation.items {
                    let context = match impl_item {
                        ImplItem::Fn(method) => {
                            format!("{prefix}{self_name}::{}", method.sig.ident)
                        }
                        _ => format!("{prefix}impl {self_name}"),
                    };
                    out.push(in_impl(context, impl_item.to_token_stream()));
                }
            }
            Item::Trait(tr) => {
                for trait_item in &tr.items {
                    let context = match trait_item {
                        TraitItem::Fn(method) => {
                            format!("{prefix}{}::{}", tr.ident, method.sig.ident)
                        }
                        _ => format!("{prefix}trait {}", tr.ident),
                    };
                    out.push(piece(context, None, trait_item.to_token_stream()));
                }
                let mut header = tr.supertraits.to_token_stream();
                header.extend(tr.generics.to_token_stream());
                out.push(piece(format!("{prefix}trait {}", tr.ident), None, header));
            }
            Item::Mod(module) => match &module.content {
                Some((_, content)) => {
                    let inner = format!("{prefix}{}::", module.ident);
                    pieces_in(content, &inner, out);
                }
                None => out.push(piece(
                    format!("{prefix}{}", item_name(item)),
                    None,
                    item.to_token_stream(),
                )),
            },
            other => out.push(piece(
                format!("{prefix}{}", item_name(other)),
                None,
                other.to_token_stream(),
            )),
        }
    }
}

/// Every piece of `file`, its inner attributes included.
fn pieces(file: &syn::File) -> Vec<Piece> {
    let inner: TokenStream = file
        .attrs
        .iter()
        .map(|attribute| attribute.to_token_stream())
        .collect();
    let mut out = vec![Piece {
        context: "the file's attributes".to_owned(),
        impl_of: None,
        trait_of: None,
        tokens: inner,
    }];
    pieces_in(&file.items, "", &mut out);
    out
}

/// Each run of sibling token trees in `tokens`, and in every group inside it.
fn levels(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
    let level: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out: Vec<Vec<TokenTree>> = level
        .iter()
        .filter_map(|tree| match tree {
            TokenTree::Group(group) => Some(group.stream()),
            _ => None,
        })
        .flat_map(levels)
        .collect();
    out.push(level);
    out
}

fn ident_at(level: &[TokenTree], at: usize) -> Option<String> {
    match level.get(at) {
        Some(TokenTree::Ident(ident)) => Some(ident.unraw().to_string()),
        _ => None,
    }
}

fn punct_at(level: &[TokenTree], at: usize, character: char) -> bool {
    matches!(level.get(at), Some(TokenTree::Punct(punct)) if punct.as_char() == character)
}

/// Whether a `::` starts at `at`.
fn path_separator_at(level: &[TokenTree], at: usize) -> bool {
    let joint = matches!(
        level.get(at),
        Some(TokenTree::Punct(punct)) if punct.as_char() == ':' && punct.spacing() == Spacing::Joint
    );
    joint && punct_at(level, at + 1, ':')
}

fn parenthesized_at(level: &[TokenTree], at: usize) -> bool {
    matches!(level.get(at), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis)
}

/// The index past the generic arguments that open at `at` with `<`.
fn past_generics(level: &[TokenTree], at: usize) -> usize {
    let mut depth = 0usize;
    let mut index = at;
    while index < level.len() {
        if punct_at(level, index, '<') {
            depth += 1;
        } else if punct_at(level, index, '>') {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index + 1;
            }
        }
        index += 1;
    }
    index
}

/// Whether the path after a type name at `at - 1` goes on to `name`: through generic
/// arguments, a closing `>` and any `::`.
fn path_goes_to(level: &[TokenTree], at: usize, name: &str) -> bool {
    let mut index = at;
    loop {
        if punct_at(level, index, '<') {
            index = past_generics(level, index);
        } else if punct_at(level, index, '>') {
            index += 1;
        } else if path_separator_at(level, index) {
            index += 2;
            if ident_at(level, index).as_deref() == Some(name) {
                return true;
            }
            if !punct_at(level, index, '<') {
                return false;
            }
        } else {
            return false;
        }
    }
}

/// Whether the identifier at `at` is the name an item definition (`fn`, `mod`) gives.
fn defined_at(level: &[TokenTree], at: usize) -> bool {
    let before = at
        .checked_sub(1)
        .and_then(|previous| ident_at(level, previous));
    matches!(before.as_deref(), Some("fn" | "mod"))
}

/// One naming a scan found: the file, the piece and what it names.
type Naming = (String, String, String);

/// Each identifier of `names` in `pieces`, past the allowance each `(piece, name)` holds.
fn named_past(
    file: &str,
    pieces: &[Piece],
    names: &BTreeSet<String>,
    allowed: &[(&str, &str, usize)],
) -> Vec<Naming> {
    let mut left: BTreeMap<(String, String), usize> = allowed
        .iter()
        .map(|(context, name, count)| (((*context).to_owned(), (*name).to_owned()), *count))
        .collect();
    let mut out = Vec::new();
    for piece in pieces {
        for ident in identifiers(piece.tokens.clone()) {
            if !names.contains(&ident) {
                continue;
            }
            let key = (piece.context.clone(), ident.clone());
            if let Some(count) = left.get_mut(&key)
                && *count > 0
            {
                *count -= 1;
                continue;
            }
            out.push((file.to_owned(), piece.context.clone(), ident));
        }
    }
    out
}

fn show(namings: &[Naming]) -> Vec<String> {
    namings
        .iter()
        .map(|(file, context, name)| format!("{file}: `{name}` in {context}"))
        .collect()
}

// The capability's surface.

/// What breaks the capability's pinned surface in the file that defines it.
fn capability_surface_findings(file: &syn::File) -> Vec<String> {
    let mut out = Vec::new();
    let mut inherent: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();
    let mut defined = false;
    for item in &file.items {
        match item {
            Item::Struct(structure) if structure.ident == WRITE_CAPABILITY => {
                defined = true;
                let derives = structure
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("derive"));
                if derives {
                    out.push("a derive on the capability".to_owned());
                }
                for field in &structure.fields {
                    if !matches!(field.vis, Visibility::Inherited) {
                        out.push(format!("a visible field: {}", field.to_token_stream()));
                    }
                }
            }
            Item::Impl(implementation)
                if type_name(&implementation.self_ty) == WRITE_CAPABILITY =>
            {
                match &implementation.trait_ {
                    Some((_, trait_path, _)) => {
                        let name = trait_path
                            .segments
                            .last()
                            .map(|segment| segment.ident.to_string())
                            .unwrap_or_default();
                        traits.push(name);
                    }
                    None => {
                        for impl_item in &implementation.items {
                            match impl_item {
                                ImplItem::Fn(method) => inherent.push(method.sig.ident.to_string()),
                                other => out.push(format!(
                                    "an inherent item other than a method: {}",
                                    other.to_token_stream()
                                )),
                            }
                        }
                    }
                }
            }
            Item::Mod(_) => {
                if !declares_under_cfg_test(std::slice::from_ref(item), "tests") {
                    out.push(format!(
                        "a module other than the unit tests: {}",
                        item_name(item)
                    ));
                }
            }
            Item::Macro(_) => out.push(format!("a macro item: {}", item_name(item))),
            _ => {}
        }
    }
    if !defined {
        out.push("no `WriteCapability` struct".to_owned());
    }
    if inherent != [GATE_ISSUED, "thing"] {
        out.push(format!("the inherent methods are {inherent:?}"));
    }
    if traits != ["Debug"] {
        out.push(format!("the trait impls are {traits:?}"));
    }
    out
}

/// Each impl on the capability, under any alias, outside the file that defines it.
fn capability_impls_elsewhere() -> Vec<String> {
    let names = with_aliases(&[WRITE_CAPABILITY], &workspace_sources());
    let mut out = Vec::new();
    for path in workspace_production() {
        if path == capability_file() {
            continue;
        }
        for piece in pieces(&parse(&path)) {
            let Some(self_name) = &piece.impl_of else {
                continue;
            };
            if piece.context.ends_with(&format!("impl {self_name}")) && names.contains(self_name) {
                out.push(format!("{}: {}", path.display(), piece.context));
            }
        }
    }
    out
}

#[test]
fn the_capability_s_surface_is_gate_issued_and_thing() {
    let file = parse(&capability_file());

    let findings = capability_surface_findings(&file);
    let elsewhere = capability_impls_elsewhere();

    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(elsewhere, Vec::<String>::new());
    let source = std::fs::read_to_string(capability_file()).expect("capability.rs reads");
    let broken = [
        (
            "pub struct WriteCapability",
            "#[derive(Clone)]\npub struct WriteCapability",
        ),
        ("    thing: T,", "    pub thing: T,"),
        (
            "    role: PhantomData<R>,",
            "    pub(crate) role: PhantomData<R>,",
        ),
        (
            "    pub fn thing(&self) -> &T {",
            "    pub fn into_thing(self) -> T {\n        self.thing\n    }\n\n    pub fn thing(&self) -> &T {",
        ),
        (
            "impl<T, R> fmt::Debug",
            "impl<T: Clone, R> Clone for WriteCapability<T, R> {\n    fn clone(&self) -> Self {\n        Self::gate_issued(self.thing.clone())\n    }\n}\n\nimpl<T, R> fmt::Debug",
        ),
        (
            "#[cfg(test)]\nmod tests;",
            "#[cfg(test)]\nmod tests;\n\nmod more {\n    impl<T, R> super::WriteCapability<T, R> {}\n}",
        ),
        ("#[cfg(test)]\nmod tests;", "mod tests;"),
    ];
    for (from, to) in broken {
        assert!(source.contains(from), "{from}");
        let planted = source.replacen(from, to, 1);
        let file = syn::parse_file(&planted).expect("the planted capability parses");

        let findings = capability_surface_findings(&file);

        assert!(!findings.is_empty(), "{to}");
    }
}

// One definition, workspace-wide.

/// The keywords whose next name an item binds: a type, a trait, a module, a function, a
/// static, a macro, a crate, or a rename (`use … as`, `extern crate … as`).
const BINDERS: [&str; 11] = [
    "struct", "enum", "union", "type", "trait", "mod", "fn", "static", "macro", "crate", "as",
];

/// The item keywords whose name a generic parameter list follows.
const GENERIC_HOLDERS: [&str; 6] = ["fn", "struct", "enum", "union", "trait", "type"];

/// The expression keywords a negation's `!(…)` may follow, which no macro call does.
const NEGATED_AFTER: [&str; 6] = ["if", "while", "return", "match", "in", "break"];

/// Whether the `>` at `at` closes angle brackets: an arrow's (`->`, `=>`) does not.
fn closes_angle_at(level: &[TokenTree], at: usize) -> bool {
    let arrow = at.checked_sub(1).is_some_and(|previous| {
        matches!(
            level.get(previous),
            Some(TokenTree::Punct(punct))
                if matches!(punct.as_char(), '-' | '=') && punct.spacing() == Spacing::Joint
        )
    });
    punct_at(level, at, '>') && !arrow
}

/// Whether the `<` at `at` opens a generic parameter list: after `impl` or `for`, or after the
/// name a `fn`, `struct`, `enum`, `union`, `trait` or `type` gives.
fn opens_generic_parameters(level: &[TokenTree], at: usize) -> bool {
    let before = at
        .checked_sub(1)
        .and_then(|previous| ident_at(level, previous));
    let keyword = at
        .checked_sub(2)
        .and_then(|previous| ident_at(level, previous));
    let directly = matches!(before.as_deref(), Some("impl" | "for"));
    let after_a_name =
        before.is_some() && keyword.is_some_and(|keyword| GENERIC_HOLDERS.contains(&&*keyword));
    punct_at(level, at, '<') && (directly || after_a_name)
}

/// The index of each generic parameter's name `level` declares, a const parameter's included.
fn generic_parameter_names(level: &[TokenTree]) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for open in (0..level.len()).filter(|at| opens_generic_parameters(level, *at)) {
        let mut depth = 0usize;
        let mut starts = false;
        for at in open..level.len() {
            if punct_at(level, at, '<') {
                depth += 1;
                starts = depth == 1;
            } else if closes_angle_at(level, at) {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            } else if depth == 1 && punct_at(level, at, ',') {
                starts = true;
            } else if depth == 1 && starts {
                let name = ident_at(level, at);
                let is_const = name.as_deref() == Some("const");
                if name.is_some() && !is_const {
                    out.insert(at);
                }
                starts = is_const;
            }
        }
    }
    out
}

/// What binds the name at `at`, if anything does: the keyword before it (`const` too, but not
/// a raw pointer's), `macro_rules!`, or the generic parameter list it sits in.
fn binder_at(level: &[TokenTree], at: usize, generic: &BTreeSet<usize>) -> Option<String> {
    let before = at
        .checked_sub(1)
        .and_then(|previous| ident_at(level, previous));
    let two_before = at
        .checked_sub(2)
        .and_then(|previous| ident_at(level, previous));
    if let Some(keyword) = &before
        && BINDERS.contains(&keyword.as_str())
    {
        return Some(format!("`{keyword}`"));
    }
    let pointer = at
        .checked_sub(2)
        .is_some_and(|previous| punct_at(level, previous, '*'))
        || two_before.as_deref() == Some("raw");
    if before.as_deref() == Some("const") && !pointer {
        return Some("`const`".to_owned());
    }
    let bang = at
        .checked_sub(1)
        .is_some_and(|previous| punct_at(level, previous, '!'));
    if bang && two_before.as_deref() == Some("macro_rules") {
        return Some("`macro_rules!`".to_owned());
    }
    generic
        .contains(&at)
        .then(|| "a generic parameter list".to_owned())
}

/// Whether the group at `at` is a macro call's input (`name!(…)`, `name![…]`, `name! {…}`) or
/// an attribute's (`#[…]`, `#![…]`).
fn macro_input_at(level: &[TokenTree], at: usize) -> bool {
    let Some(before) = at.checked_sub(1) else {
        return false;
    };
    let caller = before
        .checked_sub(1)
        .and_then(|previous| ident_at(level, previous));
    let called = punct_at(level, before, '!')
        && caller.is_some_and(|caller| !NEGATED_AFTER.contains(&caller.as_str()));
    let attribute = punct_at(level, before, '#')
        || (punct_at(level, before, '!')
            && before
                .checked_sub(1)
                .is_some_and(|previous| punct_at(level, previous, '#')));
    matches!(level.get(at), Some(TokenTree::Group(_))) && (called || attribute)
}

/// One binding a scan found: what binds it, and the name.
type Binding = (String, String);

/// Each binding of one of `names` in `tokens`, at any depth, function and macro bodies included.
/// A name inside a macro call's or an attribute's input counts as bound: the macro may bind it.
fn bindings(tokens: TokenStream, names: &BTreeSet<String>) -> Vec<Binding> {
    let mut out = Vec::new();
    bindings_in(tokens, names, false, &mut out);
    out
}

fn bindings_in(
    tokens: TokenStream,
    names: &BTreeSet<String>,
    in_macro: bool,
    out: &mut Vec<Binding>,
) {
    let level: Vec<TokenTree> = tokens.into_iter().collect();
    let generic = generic_parameter_names(&level);
    for (at, tree) in level.iter().enumerate() {
        match tree {
            TokenTree::Group(group) => {
                let inside = in_macro || macro_input_at(&level, at);
                bindings_in(group.stream(), names, inside, out);
            }
            TokenTree::Ident(ident) => {
                let name = ident.unraw().to_string();
                if !names.contains(&name) {
                    continue;
                }
                let binder = binder_at(&level, at, &generic)
                    .or_else(|| in_macro.then(|| "a macro's or an attribute's input".to_owned()));
                if let Some(binder) = binder {
                    out.push((binder, name));
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

/// Each binding of [`WRITE_CAPABILITY`] in `source`, the file at `path`, past the one `struct`
/// the capability file defines. A `use` that doesn't rename binds no new item.
fn capability_definition_findings(path: &Path, source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let names = BTreeSet::from([WRITE_CAPABILITY.to_owned()]);
    let mut found = bindings(file.to_token_stream(), &names);
    if path == capability_file()
        && let Some(definition) = found.iter().position(|(binder, _)| binder == "`struct`")
    {
        found.remove(definition);
    }
    let shown = relative(path, &crates());
    found
        .into_iter()
        .map(|(binder, name)| format!("{shown}: `{name}` bound by {binder}"))
        .collect()
}

#[test]
fn the_write_capability_is_defined_once_workspace_wide() {
    let everywhere = workspace_production();

    let offenders: Vec<String> = everywhere
        .iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            capability_definition_findings(path, &source)
        })
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    assert!(
        everywhere.contains(&capability_file()),
        "the scan reads kernel's capability"
    );
    let names = BTreeSet::from([WRITE_CAPABILITY.to_owned()]);
    let defined = bindings(parse(&capability_file()).to_token_stream(), &names);
    let one_struct = vec![("`struct`".to_owned(), WRITE_CAPABILITY.to_owned())];
    assert_eq!(defined, one_struct, "capability.rs defines it once");
    let fact = crates().join("workflow/src/elements/commission/fact.rs");
    let fact_source = std::fs::read_to_string(&fact).expect("fact.rs reads");
    let look_alike = format!(
        "{fact_source}\npub struct WriteCapability<T, R>(pub T, pub core::marker::PhantomData<R>);\n"
    );
    let planted = capability_definition_findings(&fact, &look_alike);
    assert_eq!(
        planted,
        ["workflow/src/elements/commission/fact.rs: `WriteCapability` bound by `struct`"]
    );
    let capability_source =
        std::fs::read_to_string(capability_file()).expect("capability.rs reads");
    let second_in_kernel =
        format!("{capability_source}\nmod more {{ pub struct WriteCapability; }}\n");
    let elsewhere = crates().join("workflow/src/elements/commission/capability.rs");
    let refused = [
        "pub enum WriteCapability {}",
        "pub union WriteCapability { thing: u8 }",
        "pub type WriteCapability<T, R> = (T, R);",
        "pub trait WriteCapability {}",
        "pub mod WriteCapability {}",
        "use kernel::ports::Creator as WriteCapability;",
        "pub use kernel::ports::{Creator, New as WriteCapability};",
        "extern crate kernel as WriteCapability;",
        "trait Gate { type WriteCapability; }",
        "impl Gate for Den { type WriteCapability = u8; }",
        "fn forge<WriteCapability>(thing: WriteCapability) {}",
        "impl<F: Fn() -> u8, WriteCapability> Holder for F {}",
        "struct Holder<const WriteCapability: usize>;",
        "pub const WriteCapability: u8 = 0;",
        "pub static WriteCapability: u8 = 0;",
        "pub fn WriteCapability() {}",
        "fn lend() { struct WriteCapability<T, R>(pub T, pub PhantomData<R>); }",
        "mod inner { mod deeper { pub struct r#WriteCapability; } }",
        "macro_rules! forge { () => { pub struct WriteCapability<T, R>(pub T, pub PhantomData<R>); }; }",
        "macro_rules! named { ($name:ident) => { pub struct $name; }; }\nnamed!(WriteCapability);",
        "macro_rules! WriteCapability { () => {}; }",
        "#[make_type(WriteCapability)]\nfn lend() {}",
    ];
    let mut planted_files: Vec<(PathBuf, String)> = refused
        .into_iter()
        .map(|source| (elsewhere.clone(), source.to_owned()))
        .collect();
    planted_files.push((capability_file(), second_in_kernel));
    for (path, source) in planted_files {
        let found = capability_definition_findings(&path, &source);

        assert!(!found.is_empty(), "{}: {source}", path.display());
    }
    let kept = [
        "pub use kernel::ports::WriteCapability;",
        "use kernel::{ports::{Creator, New, WriteCapability}};",
        "pub use kernel::ports::*;",
        "fn take(capability: &WriteCapability<CommissionId, AsOwner>) -> Vec<WriteCapability<u8, Creator>> { todo!() }",
        "impl<T, R, F: Fn() -> WriteCapability<T, R>> Holder<T, R> for Vec<WriteCapability<T, R>> where T: Fn(u8) -> WriteCapability<T, R> {}",
        "struct Holder<T = WriteCapability<u8, Creator>>(T);",
        "type Lent<'a> = &'a WriteCapability<u8, Creator>;",
        "fn raw(capability: *const WriteCapability<u8, Creator>) {}",
        "fn check(capability: &WriteCapability<u8, Creator>) -> bool { if !(WriteCapability::thing(capability) == &0) { return false; } true }",
    ];
    for source in kept {
        let found = capability_definition_findings(&elsewhere, source);

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

// One issuer, workspace-wide.

/// The allowance the issuer scan gives the file at `path`: the definition in the capability
/// file, and one call in [`ISSUE`] in the gate file.
fn issuer_allowance(path: &Path) -> Vec<(&'static str, &'static str, usize)> {
    if path == capability_file() {
        return vec![("WriteCapability::gate_issued", GATE_ISSUED, 1)];
    }
    if path == application_src().join("den/gate.rs") {
        return vec![(ISSUE, GATE_ISSUED, 1)];
    }
    Vec::new()
}

/// Each naming of the issuer in `source`, at `path`, past its allowance.
fn issuer_findings(path: &Path, source: &str, names: &BTreeSet<String>) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let found = named_past(
        &path.display().to_string(),
        &pieces(&file),
        names,
        &issuer_allowance(path),
    );
    show(&found)
}

#[test]
fn gate_issued_is_defined_once_and_called_only_in_issue() {
    let names = with_aliases(&[GATE_ISSUED], &workspace_sources());
    let everywhere = workspace_production();

    let offenders: Vec<String> = everywhere
        .iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            issuer_findings(path, &source, &names)
        })
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let gate_file = application_src().join("den/gate.rs");
    for (path, context) in [
        (capability_file(), "WriteCapability::gate_issued"),
        (gate_file.clone(), ISSUE),
    ] {
        assert!(everywhere.contains(&path), "{}", path.display());
        let named: usize = pieces(&parse(&path))
            .iter()
            .filter(|piece| piece.context == context)
            .map(|piece| {
                identifiers(piece.tokens.clone())
                    .iter()
                    .filter(|ident| *ident == GATE_ISSUED)
                    .count()
            })
            .sum();
        assert_eq!(named, 1, "{} {context}", path.display());
    }
    let workflow = crates().join("workflow/src/elements/commission/capability.rs");
    let refused = [
        (
            workflow.clone(),
            "pub fn issued(id: CommissionId) -> WriteCapability<CommissionId, AsOwner> { WriteCapability::gate_issued(id) }",
        ),
        (
            workflow.clone(),
            "pub fn issuer() -> fn(u32) -> WriteCapability<u32, Creator> { WriteCapability::gate_issued }",
        ),
        (
            workflow.clone(),
            "macro_rules! forge { ($thing:expr) => { WriteCapability::gate_issued($thing) }; }",
        ),
        (
            gate_file.clone(),
            "fn issue<R: RoleTable>(grant: &Grant) -> Option<WriteCapability<R::Thing, R>> { let thing = R::granted(grant)?; WriteCapability::gate_issued(R::granted(grant)?); Some(WriteCapability::gate_issued(thing)) }",
        ),
        (
            gate_file.clone(),
            "fn lend(thing: CommissionId) { let capability = WriteCapability::<_, AsOwner>::gate_issued(thing); }",
        ),
        (
            gate_file.clone(),
            "mod inner { fn issue(thing: CommissionId) { WriteCapability::<_, AsOwner>::gate_issued(thing); } }",
        ),
        (
            capability_file(),
            "impl<T, R> WriteCapability<T, R> { pub fn gate_issued(thing: T) -> Self { Self { thing, role: PhantomData } } pub fn again(thing: T) -> Self { Self::gate_issued(thing) } }",
        ),
        (
            crates().join("api/src/routes/commissions/rename.rs"),
            "async fn rename(id: CommissionId) { let capability = domain::ports::WriteCapability::<_, AsOwner>::gate_issued(id); }",
        ),
    ];
    for (path, source) in refused {
        let found = issuer_findings(&path, source, &names);

        assert!(!found.is_empty(), "{}: {source}", path.display());
    }
}

// The issuer's callers, inside the den.

/// The gate's role table: named, and so implemented, only in [`GATE_FILE`].
const ROLE_TABLE: &str = "RoleTable";

/// The gate file, relative to the use-case layer's sources.
const GATE_FILE: &str = "den/gate.rs";

/// The gate's one generic lender: the one [`ISSUE`] call.
const LEND_AS: &str = "lend_as";

/// The gate's lender: one [`LEND_AS`] call per leaf act a driver can reach.
const LEND: &str = "lend";

/// The den's acts: each arm a driver can reach is an owning module's own act enum.
const DEN_ACT: &str = "DenAct";

/// The commission's acts, one leaf act each.
const COMMISSION_ACT: &str = "CommissionAct";

/// The file of the commission's acts, relative to the use-case layer's sources.
const COMMISSION_ACT_FILE: &str = "commission/act.rs";

/// The den module's production files: `den.rs` and those under `den/`, each with its path
/// relative to the use-case layer's sources.
fn den_production() -> Vec<(String, PathBuf)> {
    let src = application_src();
    walk(&src)
        .production
        .into_iter()
        .map(|path| (relative(&path, &src), path))
        .filter(|(file, _)| file == "den.rs" || file.starts_with("den/"))
        .collect()
}

/// The variants of the one enum named `name` in `file` that a driver can reach: those not
/// under `#[cfg(test)]`.
fn production_variants(file: &syn::File, name: &str) -> Vec<String> {
    let enums: Vec<&syn::ItemEnum> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(enumeration) if enumeration.ident == name => Some(enumeration),
            _ => None,
        })
        .collect();
    assert_eq!(enums.len(), 1, "one `{name}`");
    enums[0]
        .variants
        .iter()
        .filter(|variant| !under_cfg_test(&variant.attrs))
        .map(|variant| variant.ident.to_string())
        .collect()
}

/// The leaf acts a driver can reach: `DenAct`'s one production arm is the commission's, and
/// each of `CommissionAct`'s production variants is one leaf act.
fn production_leaf_acts(den_act: &syn::File, commission_act: &syn::File) -> usize {
    let arms = production_variants(den_act, DEN_ACT);
    assert_eq!(
        arms,
        ["Commission"],
        "a new arm counts its own leaf acts here"
    );
    production_variants(commission_act, COMMISSION_ACT).len()
}

/// The allowance the issuer-caller scan gives `file`: in [`GATE_FILE`], `issue`'s definition
/// and one call in [`LEND_AS`]; `lend_as`'s definition and `leaves` calls in [`LEND`].
fn issue_allowance(file: &str, leaves: usize) -> Vec<(&'static str, &'static str, usize)> {
    if file != GATE_FILE {
        return Vec::new();
    }
    vec![
        (ISSUE, ISSUE, 1),
        (LEND_AS, ISSUE, 1),
        (LEND_AS, LEND_AS, 1),
        (LEND, LEND_AS, leaves),
    ]
}

/// Each naming in den file `file` of the issuer or the generic lender (`issues`) past its
/// allowance, and of the role table (`roles`) outside [`GATE_FILE`].
fn issue_caller_findings(
    file: &str,
    source: &str,
    issues: &BTreeSet<String>,
    roles: &BTreeSet<String>,
    leaves: usize,
) -> Vec<String> {
    let parsed = syn::parse_file(source).expect("the source parses");
    let names: BTreeSet<String> = if file == GATE_FILE {
        issues.clone()
    } else {
        issues.union(roles).cloned().collect()
    };
    let found = named_past(
        file,
        &pieces(&parsed),
        &names,
        &issue_allowance(file, leaves),
    );
    show(&found)
}

#[test]
fn issue_is_called_only_by_the_lenders_and_the_role_table_stays_in_the_gate() {
    let files = den_production();
    let sources: Vec<String> = files
        .iter()
        .map(|(_, path)| std::fs::read_to_string(path).expect("a readable source"))
        .collect();
    let issues = with_aliases(&[ISSUE, LEND_AS], &sources);
    let roles = with_aliases(&[ROLE_TABLE], &sources);
    let leaves = production_leaf_acts(
        &parse(&application_src().join("den/act.rs")),
        &parse(&application_src().join(COMMISSION_ACT_FILE)),
    );

    let offenders: Vec<String> = files
        .iter()
        .zip(&sources)
        .flat_map(|((file, _), source)| {
            issue_caller_findings(file, source, &issues, &roles, leaves)
        })
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let read: Vec<&str> = files.iter().map(|(file, _)| file.as_str()).collect();
    for file in ["den.rs", GATE_FILE, "den/write.rs", "den/act.rs"] {
        assert!(read.contains(&file), "the scan reads {file}: {read:?}");
    }
    let gate = parse(&application_src().join(GATE_FILE));
    let gate_pieces = pieces(&gate);
    for context in [ISSUE, LEND_AS, LEND] {
        let held = gate_pieces.iter().any(|piece| piece.context == context);
        assert!(held, "{GATE_FILE} holds {context}");
    }
    let lender = "fn issue<R: RoleTable>(grant: &Grant) -> Option<WriteCapability<R::Thing, R>> { let thing = R::granted(grant)?; Some(WriteCapability::gate_issued(thing)) }\nasync fn lend_as<H: Handler>(handler: &H, grant: &Grant) { let Some(capability) = issue::<H::Role>(grant) else { return; }; }\nasync fn lend(act: DenAct, grant: &Grant) { match act { DenAct::Commission(CommissionAct::Rename(rename)) => lend_as(&rename, grant).await, DenAct::Commission(CommissionAct::Create(create)) => lend_as(&create, grant).await } }\nimpl RoleTable for AsOwner { type Thing = CommissionId; }";
    let allowed = issue_caller_findings(GATE_FILE, lender, &issues, &roles, 2);
    assert_eq!(allowed, Vec::<String>::new());
    let refused = [
        (
            "den/write.rs",
            "pub(in crate::den) async fn written(unit: Unit<'_>, grant: Grant) { let _early = crate::den::gate::issue::<AsOwner>(&grant); }",
            0,
        ),
        (
            "den/write.rs",
            "use crate::den::gate::issue as mint;\nasync fn written(grant: Grant) { let _early = mint::<AsOwner>(&grant); }",
            0,
        ),
        (
            "den/write.rs",
            "fn early(grant: &Grant) { let forge = crate::den::gate::issue::<Creator>; forge(grant); }",
            0,
        ),
        ("den/write.rs", "fn issue() {}", 0),
        ("den.rs", "pub(crate) use gate::issue;", 0),
        (
            GATE_FILE,
            "async fn lend(act: DenAct, grant: &Grant) { issue::<AsOwner>(grant); }",
            0,
        ),
        (
            GATE_FILE,
            "async fn lend(act: DenAct, grant: &Grant) { match act { DenAct::Rename(_) => { issue::<AsOwner>(grant); issue::<Creator>(grant); } } }",
            1,
        ),
        (
            GATE_FILE,
            "#[cfg(test)]\nasync fn lend_for_test<R: RoleTable>(grant: &Grant) { issue::<R>(grant); issue::<AsOwner>(grant); }",
            0,
        ),
        (
            GATE_FILE,
            "async fn lend_as<H: Handler>(handler: &H, grant: &Grant) { issue::<H::Role>(grant); issue::<Creator>(grant); }",
            0,
        ),
        (
            GATE_FILE,
            "async fn lend(act: DenAct, grant: &Grant) { match act { DenAct::Commission(CommissionAct::Create(create)) => { lend_as(&create, grant).await; lend_as(&create, grant).await; } } }",
            1,
        ),
        (
            GATE_FILE,
            "#[cfg(test)]\nasync fn lend_for_test(act: &TestAct, grant: &Grant) { lend_as(&AsOwnerForTest(act), grant).await; }",
            0,
        ),
        (
            GATE_FILE,
            "async fn walk(grant: &Grant) { lend_as(&create, grant).await; }",
            0,
        ),
        (
            "den/write.rs",
            "fn early(grant: &Grant, act: &create::Act) { crate::den::gate::lend_as(act, grant); }",
            0,
        ),
        (
            GATE_FILE,
            "async fn walk(grant: &Grant) { let early = issue::<AsOwner>(grant); }",
            0,
        ),
        (
            GATE_FILE,
            "mod inner { async fn lend(grant: &Grant) { issue::<AsOwner>(grant); } }",
            1,
        ),
        (
            GATE_FILE,
            "impl DenWriteHost<'_> { fn lend(grant: &Grant) { issue::<AsOwner>(grant); } }",
            1,
        ),
        (
            GATE_FILE,
            "macro_rules! early { ($grant:expr) => { issue::<AsOwner>($grant) }; }",
            0,
        ),
        (
            "den/act.rs",
            "impl RoleTable for Forged { type Thing = CommissionId; fn granted(grant: &Grant) -> Option<CommissionId> { None } }",
            0,
        ),
        (
            "den/act.rs",
            "use crate::den::gate::RoleTable as Table;\nimpl Table for Forged {}",
            0,
        ),
        (
            "den/write.rs",
            "fn lend<R: crate::den::gate::RoleTable>(grant: &Grant) {}",
            0,
        ),
    ];
    for (file, source, fixture_leaves) in refused {
        let fixture_sources = [source.to_owned()];
        let fixture_issues = with_aliases(&[ISSUE, LEND_AS], &fixture_sources);
        let fixture_roles = with_aliases(&[ROLE_TABLE], &fixture_sources);

        let found = issue_caller_findings(
            file,
            source,
            &fixture_issues,
            &fixture_roles,
            fixture_leaves,
        );

        assert!(!found.is_empty(), "{file}: {source}");
    }
    let den_act = "pub(crate) enum DenAct {\n    Commission(CommissionAct),\n    #[cfg(test)]\n    TestOnly(TestAct),\n}";
    let commission_act =
        "pub(crate) enum CommissionAct {\n    Create(create::Act),\n    Rename(rename::Act),\n}";
    let den_act = syn::parse_file(den_act).expect("the planted act parses");
    let commission_act = syn::parse_file(commission_act).expect("the planted act parses");
    assert_eq!(production_leaf_acts(&den_act, &commission_act), 2);
}

// The lent view.

/// The view the gate lends a handler.
const LENT: &str = "Lent";

/// The lent view's field holding the latch every method passes its call through.
const LATCH: &str = "latch";

/// The lent view's field holding the unlatched calls.
const VIEW: &str = "view";

/// The latch's one method.
const PASS: &str = "pass";

/// The role an inherent impl on the lent view is for: its self type's role argument, or
/// "every role" when that argument is the impl's own type parameter.
fn lent_role(implementation: &syn::ItemImpl) -> String {
    let Type::Path(path) = &*implementation.self_ty else {
        return "an unnamed type".to_owned();
    };
    let Some(last) = path.path.segments.last() else {
        return "an unnamed type".to_owned();
    };
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return "no role".to_owned();
    };
    let parameters: BTreeSet<String> = implementation
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect();
    let role = arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(role) => Some(type_name(role)),
        _ => None,
    });
    match role {
        Some(role) if parameters.contains(&role) => "every role".to_owned(),
        Some(role) => role,
        None => "no role".to_owned(),
    }
}

/// Every item in `items` and in each inline module among them, at any depth.
fn every_item(items: &[Item]) -> Vec<&Item> {
    items
        .iter()
        .flat_map(|item| {
            let mut found = vec![item];
            if let Item::Mod(module) = item
                && let Some((_, content)) = &module.content
            {
                found.extend(every_item(content));
            }
            found
        })
        .collect()
}

/// `seeds`, and every name `file` binds to one of them, through any chain of aliases.
fn aliases_in_file(seeds: &[&str], file: &syn::File) -> BTreeSet<String> {
    let source = file.to_token_stream().to_string();
    with_aliases(seeds, &[source])
}

/// Every method the lent view defines in `file`, inline modules included, with the role its
/// impl is for, and what else breaks its pinned surface there: a visible field, a derive, a
/// trait impl, an inherent item other than a method. An alias of the view counts as the view.
fn lent_surface(file: &syn::File) -> (Vec<(String, String)>, Vec<String>) {
    let lents = aliases_in_file(&[LENT], file);
    let mut methods = Vec::new();
    let mut out = Vec::new();
    let mut defined = 0;
    for item in every_item(&file.items) {
        match item {
            Item::Struct(structure) if structure.ident == LENT => {
                defined += 1;
                let derives = structure
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("derive"));
                if derives {
                    out.push("a derive on the lent view".to_owned());
                }
                for field in &structure.fields {
                    if !matches!(field.vis, Visibility::Inherited) {
                        out.push(format!("a visible field: {}", field.to_token_stream()));
                    }
                }
            }
            Item::Impl(implementation) if lents.contains(&type_name(&implementation.self_ty)) => {
                if let Some((_, trait_path, _)) = &implementation.trait_ {
                    out.push(format!("a trait impl: {}", trait_path.to_token_stream()));
                    continue;
                }
                let role = lent_role(implementation);
                for impl_item in &implementation.items {
                    match impl_item {
                        ImplItem::Fn(method) => {
                            methods.push((method.sig.ident.to_string(), role.clone()));
                        }
                        other => out.push(format!(
                            "an inherent item other than a method: {}",
                            other.to_token_stream()
                        )),
                    }
                }
            }
            _ => {}
        }
    }
    if defined != 1 {
        out.push(format!("{defined} `{LENT}` structs"));
    }
    (methods, out)
}

/// What breaks the lent view's pinned surface in `file`: `create` for a creator's view, `lent`
/// for every role's, and nothing else.
fn lent_surface_findings(file: &syn::File) -> Vec<String> {
    let (methods, mut out) = lent_surface(file);
    let pinned = [
        ("create".to_owned(), "Creator".to_owned()),
        ("lent".to_owned(), "every role".to_owned()),
    ];
    if methods != pinned {
        out.push(format!("the methods and their roles are {methods:?}"));
    }
    out
}

/// Each impl on the lent view, under any alias, outside the gate file.
fn lent_impls_elsewhere() -> Vec<String> {
    let src = application_src();
    let sources: Vec<(PathBuf, String)> = walk(&src)
        .production
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            (path, source)
        })
        .collect();
    let texts: Vec<String> = sources.iter().map(|(_, source)| source.clone()).collect();
    let names = with_aliases(&[LENT], &texts);
    let mut out = Vec::new();
    for (path, source) in &sources {
        let file = relative(path, &src);
        if file == GATE_FILE {
            continue;
        }
        let parsed = syn::parse_file(source).expect("the source parses");
        for piece in pieces(&parsed) {
            let Some(self_name) = &piece.impl_of else {
                continue;
            };
            if piece.context.ends_with(&format!("impl {self_name}")) && names.contains(self_name) {
                out.push(format!("{file}: {}", piece.context));
            }
        }
    }
    out
}

#[test]
fn the_lent_view_s_surface_is_create_and_lent() {
    let gate = parse(&application_src().join(GATE_FILE));

    let findings = lent_surface_findings(&gate);
    let elsewhere = lent_impls_elsewhere();

    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(elsewhere, Vec::<String>::new());
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let widened = format!("{WIDENED_MODULE}{AFTER_THE_LENT_VIEW}");
    let broken = [
        (
            "impl<R: RoleTable> Lent<'_, R> {",
            "impl<R: RoleTable> Lent<'_, R> {\n    pub(crate) async fn grant_view(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.grant_view().await)\n    }\n",
        ),
        (
            "    view: View<'a, R>,",
            "    pub(crate) view: View<'a, R>,",
        ),
        (
            "pub(crate) struct Lent<'a, R: RoleTable> {",
            "#[derive(Default)]\npub(crate) struct Lent<'a, R: RoleTable> {",
        ),
        ("impl Lent<'_, Creator> {", "impl Lent<'_, AsOwner> {"),
        (
            "impl Lent<'_, Creator> {",
            "impl<R: RoleTable> std::ops::Deref for Lent<'_, R> {\n    type Target = Box<dyn CommissionRepo>;\n    fn deref(&self) -> &Self::Target {\n        &self.view.repo\n    }\n}\n\nimpl Lent<'_, Creator> {",
        ),
        (
            "impl Lent<'_, Creator> {",
            "impl Lent<'_, AsOwner> {\n    pub(crate) async fn create(&mut self, commission: &NewCommission) -> anyhow::Result<()> {\n        self.latch.pass(self.view.create(commission).await)\n    }\n}\n\nimpl Lent<'_, Creator> {",
        ),
        (AFTER_THE_LENT_VIEW, widened.as_str()),
        (
            AFTER_THE_LENT_VIEW,
            "mod outer {\n    mod inner {\n        impl super::super::Lent<'_, AsOwner> {\n            pub(crate) async fn grant(&mut self) -> anyhow::Result<()> {\n                self.latch.pass(self.view.grant().await)\n            }\n        }\n    }\n}\n\n/// What a lent handler answered",
        ),
        (
            AFTER_THE_LENT_VIEW,
            "type Owned<'a> = Lent<'a, AsOwner>;\n\nimpl Owned<'_> {\n    pub(crate) async fn grant(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.grant().await)\n    }\n}\n\n/// What a lent handler answered",
        ),
    ];
    for (from, to) in broken {
        assert!(source.contains(from), "{from}");
        let planted = source.replacen(from, to, 1);
        let file = syn::parse_file(&planted).expect("the planted gate parses");

        let findings = lent_surface_findings(&file);

        assert!(!findings.is_empty(), "{to}");
    }
}

/// Whether `expr` is `self.<field>`.
fn is_self_field(expr: &syn::Expr, field: &str) -> bool {
    let syn::Expr::Field(access) = expr else {
        return false;
    };
    let syn::Member::Named(member) = &access.member else {
        return false;
    };
    let syn::Expr::Path(base) = &*access.base else {
        return false;
    };
    base.path.is_ident("self") && member == field
}

/// Whether `method`'s whole body is `self.latch.pass(self.view.<its own name>(…).await)`.
fn passes_its_call_through_the_latch(method: &syn::ImplItemFn) -> bool {
    let [syn::Stmt::Expr(syn::Expr::MethodCall(pass), None)] = method.block.stmts.as_slice() else {
        return false;
    };
    let [syn::Expr::Await(awaited)] = pass.args.iter().collect::<Vec<_>>().as_slice() else {
        return false;
    };
    let syn::Expr::MethodCall(inner) = &*awaited.base else {
        return false;
    };
    is_self_field(&pass.receiver, LATCH)
        && pass.method == PASS
        && is_self_field(&inner.receiver, VIEW)
        && inner.method == method.sig.ident
}

/// Each method of the lent view in `file`, inline modules and aliases included, whose body
/// doesn't pass its own inner call through the latch.
fn unlatched_methods(file: &syn::File) -> Vec<String> {
    let lents = aliases_in_file(&[LENT], file);
    every_item(&file.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Impl(implementation)
                if lents.contains(&type_name(&implementation.self_ty))
                    && implementation.trait_.is_none() =>
            {
                Some(implementation)
            }
            _ => None,
        })
        .flat_map(|implementation| &implementation.items)
        .filter_map(|impl_item| match impl_item {
            ImplItem::Fn(method) if !passes_its_call_through_the_latch(method) => {
                Some(format!("{LENT}::{}", method.sig.ident))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_lent_method_goes_through_the_latch() {
    let gate = parse(&application_src().join(GATE_FILE));

    let unlatched = unlatched_methods(&gate);

    assert_eq!(unlatched, Vec::<String>::new());
    let (methods, _) = lent_surface(&gate);
    assert_eq!(methods.len(), 2, "the scan reads both lent methods");
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let widened = format!("{WIDENED_MODULE}{AFTER_THE_LENT_VIEW}");
    let broken = [
        (AFTER_THE_LENT_VIEW, widened.as_str()),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.view.lent().await",
        ),
        (
            "        self.latch.pass(self.view.create(commission).await)",
            "        self.view.create(commission).await",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.latch.pass(self.view.create(&NewCommission { maturity: None, deadline: None }).await)",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        let read = self.view.lent().await;\n        self.latch.pass(read)",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.latch.pass(self.view.lent().await);\n        self.view.lent().await",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.view.latch.pass(self.view.lent().await)",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.latch.keep(self.view.lent().await)",
        ),
    ];
    for (from, to) in broken {
        assert!(source.contains(from), "{from}");
        let planted = source.replacen(from, to, 1);
        let file = syn::parse_file(&planted).expect("the planted gate parses");

        let unlatched = unlatched_methods(&file);

        assert!(!unlatched.is_empty(), "{to}");
    }
}

/// The lent view's inner view: only [`LENT`]'s methods and [`LEND_AS`] reach it.
const VIEW_TYPE: &str = "View";

/// The inner view's field holding the commission repo.
const REPO: &str = "repo";

/// The struct literals and patterns of the views [`LEND_AS`] holds: it builds the inner view
/// and the lent view, then takes the lent view apart.
const LENDER_STRUCTS: [(&str, &str, usize); 2] = [(LEND_AS, VIEW_TYPE, 1), (LEND_AS, LENT, 2)];

/// The anchor in the gate file a planted item goes before: the item after the lent view.
const AFTER_THE_LENT_VIEW: &str = "/// What a lent handler answered";

/// A nested module in the gate file that widens an owner's lent view: a module there sees the
/// private fields of `Lent` and `View`.
const WIDENED_MODULE: &str = "mod widened {\n    use super::*;\n    use domain::elements::commission::GrantLevel;\n\n    impl Lent<'_, AsOwner> {\n        pub(crate) async fn grant_view_to(&mut self, user: &UserId) -> anyhow::Result<()> {\n            let commission = *self.view.capability.thing();\n            self.latch.pass(\n                self.view\n                    .repo\n                    .grant_view(&commission, user, GrantLevel::Total)\n                    .await,\n            )\n        }\n    }\n}\n\n";

/// Whether `piece` sits in an impl of a view in `views` at the file's top level.
fn held_by_a_view(piece: &Piece, views: &BTreeSet<String>) -> bool {
    piece.impl_of.as_ref().is_some_and(|name| {
        let in_the_impl = piece.context == format!("impl {name}")
            || piece.context.starts_with(&format!("{name}::"));
        views.contains(name) && in_the_impl
    })
}

/// Whether the identifier at `at` is the `mod` keyword, a raw `r#mod` aside.
fn module_keyword_at(level: &[TokenTree], at: usize) -> bool {
    matches!(level.get(at), Some(TokenTree::Ident(ident)) if *ident == "mod")
}

/// Whether a `.view` or `.repo` field access (or a method of either name) starts at `at`.
fn view_field_at(level: &[TokenTree], at: usize) -> bool {
    let field = ident_at(level, at + 1);
    let named = matches!(field.as_deref(), Some(name) if name == VIEW || name == REPO);
    punct_at(level, at, '.') && !punct_at(level, at.wrapping_sub(1), '.') && named
}

/// Whether the identifier at `at`, a view in `views` (or `Self` in a view's impl), opens a
/// struct literal or pattern: a brace group next, or after a turbofish.
fn view_struct_at(
    level: &[TokenTree],
    at: usize,
    views: &BTreeSet<String>,
    held: bool,
) -> Option<String> {
    let name = ident_at(level, at)?;
    let a_view = views.contains(&name) || (held && name == "Self");
    let defines = matches!(
        ident_at(level, at.wrapping_sub(1)).as_deref(),
        Some("struct" | "enum" | "union" | "impl")
    );
    let returned =
        punct_at(level, at.wrapping_sub(1), '>') && punct_at(level, at.wrapping_sub(2), '-');
    let braces_at = if path_separator_at(level, at + 1) && punct_at(level, at + 3, '<') {
        past_generics(level, at + 3)
    } else {
        at + 1
    };
    let braced = matches!(level.get(braces_at), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace);
    (a_view && braced && !defines && !returned).then_some(name)
}

/// Each view of `views` a struct literal or pattern in `piece` names, in order.
fn view_structs(piece: &Piece, views: &BTreeSet<String>) -> Vec<String> {
    let held = held_by_a_view(piece, views);
    levels(piece.tokens.clone())
        .iter()
        .flat_map(|level| (0..level.len()).filter_map(|at| view_struct_at(level, at, views, held)))
        .collect()
}

/// What reaches the private fields of the lent view or its inner view in the gate file
/// `file`, besides their own methods: any module but the unit tests' (a module there sees
/// both views' fields), a `.view` or `.repo` outside an impl of either view at the file's top
/// level, and a struct literal or pattern of either past [`LENDER_STRUCTS`]. Macro bodies and
/// function bodies included.
fn lent_reach_findings(file: &syn::File) -> Vec<String> {
    let views = aliases_in_file(&[LENT, VIEW_TYPE], file);
    let mut out = Vec::new();
    for item in &file.items {
        let Item::Mod(_) = item else {
            let nested = levels(item.to_token_stream())
                .iter()
                .any(|level| (0..level.len()).any(|at| module_keyword_at(level, at)));
            if nested {
                out.push(format!("a module inside {}", item_name(item)));
            }
            continue;
        };
        if !declares_under_cfg_test(std::slice::from_ref(item), "tests") {
            out.push(format!(
                "a module other than the unit tests: {}",
                item_name(item)
            ));
        }
    }
    let mut left: BTreeMap<(String, String), usize> = LENDER_STRUCTS
        .iter()
        .map(|(context, name, count)| (((*context).to_owned(), (*name).to_owned()), *count))
        .collect();
    for piece in pieces(file) {
        let reached = !held_by_a_view(&piece, &views)
            && levels(piece.tokens.clone())
                .iter()
                .any(|level| (0..level.len()).any(|at| view_field_at(level, at)));
        if reached {
            out.push(format!("a view field reached in {}", piece.context));
        }
        for name in view_structs(&piece, &views) {
            let key = (piece.context.clone(), name.clone());
            if let Some(count) = left.get_mut(&key)
                && *count > 0
            {
                *count -= 1;
                continue;
            }
            out.push(format!(
                "a `{name}` struct literal or pattern in {}",
                piece.context
            ));
        }
    }
    out
}

#[test]
fn nothing_else_in_the_gate_file_reaches_the_lent_view_s_fields() {
    let gate = parse(&application_src().join(GATE_FILE));

    let findings = lent_reach_findings(&gate);

    assert_eq!(findings, Vec::<String>::new());
    let views = aliases_in_file(&[LENT, VIEW_TYPE], &gate);
    let in_the_lender: Vec<String> = pieces(&gate)
        .iter()
        .filter(|piece| piece.context == LEND_AS)
        .flat_map(|piece| view_structs(piece, &views))
        .collect();
    assert_eq!(
        in_the_lender,
        [VIEW_TYPE, LENT, LENT],
        "the scan reads the view structs {LEND_AS} holds"
    );
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let free_function = "pub(crate) async fn grant_view_to(lent: &mut Lent<'_, AsOwner>, user: &UserId) -> anyhow::Result<()> {\n    let commission = *lent.view.capability.thing();\n    lent.view.repo.grant_view(&commission, user, GrantLevel::Total).await\n}\n\n";
    let destructured = "pub(crate) async fn grant_view_to(lent: &mut Lent<'_, AsOwner>, user: &UserId) -> anyhow::Result<()> {\n    let Lent { view, .. } = lent;\n    let View { repo, capability, .. } = view;\n    repo.grant_view(capability.thing(), user, GrantLevel::Total).await\n}\n\n";
    let planted_items = [
        WIDENED_MODULE,
        free_function,
        destructured,
        "mod widened;\n\n",
        "#[cfg(test)]\nmod tests_too;\n\n",
        "fn widen() {\n    mod inner {}\n}\n\n",
        "macro_rules! widen {\n    ($lent:expr) => {\n        $lent.view.repo\n    };\n}\n\n",
        "fn widen(view: &mut View<'_, AsOwner>) -> &mut Box<dyn CommissionRepo + '_> {\n    &mut view.repo\n}\n\n",
        "fn widen<'a>(lent: Lent<'a, AsOwner>) -> View<'a, AsOwner> {\n    let Lent::<'a, AsOwner> { view, .. } = lent;\n    view\n}\n\n",
        "type Bare<'a> = View<'a, AsOwner>;\n\nfn widen(view: Bare<'_>) {\n    let Bare { repo, .. } = view;\n}\n\n",
        "impl View<'_, AsOwner> {\n    fn widen(self) -> Box<dyn CommissionRepo> {\n        let Self { repo, .. } = self;\n        repo\n    }\n}\n\n",
        "fn widen(lent: &mut Lent<'_, AsOwner>) {\n    let reach = || lent.view.capability.thing();\n}\n\n",
    ];
    let lender_taken_apart = (
        "    drop(view);\n",
        "    let View { repo, .. } = view;\n    drop(repo);\n",
    );
    assert!(source.contains(AFTER_THE_LENT_VIEW));
    let mut planted: Vec<String> = planted_items
        .iter()
        .map(|item| {
            source.replacen(
                AFTER_THE_LENT_VIEW,
                &format!("{item}{AFTER_THE_LENT_VIEW}"),
                1,
            )
        })
        .collect();
    assert!(source.contains(lender_taken_apart.0));
    planted.push(source.replacen(lender_taken_apart.0, lender_taken_apart.1, 1));
    for planted_gate in planted {
        let file = syn::parse_file(&planted_gate).expect("the planted gate parses");

        let findings = lent_reach_findings(&file);

        assert!(!findings.is_empty(), "{planted_gate}");
    }
}

// Every compiled source is a file the walker reads.

/// The macro that compiles another file's text in place.
const INCLUDE: &str = "include";

/// The `include!` calls allowed, each in its generated-code file (relative to the crates
/// folder), by count: the embedded migrations and the contract's two generated files.
const ALLOWED_INCLUDES: [(&str, usize); 2] = [
    ("adapter-pg/src/lib.rs", 1),
    ("api/src/generated/mod.rs", 2),
];

/// Whether the attribute tokens `meta` (inside `#[…]`) move a module: `path = …` itself, or
/// inside a `cfg_attr`, nested ones included.
fn moves_a_module(meta: &[TokenTree]) -> bool {
    match ident_at(meta, 0).as_deref() {
        Some("path") => punct_at(meta, 1, '='),
        Some("cfg_attr") => {
            let Some(TokenTree::Group(group)) = meta.get(1) else {
                return false;
            };
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            inner
                .split(|tree| matches!(tree, TokenTree::Punct(punct) if punct.as_char() == ','))
                .skip(1)
                .any(moves_a_module)
        }
        _ => false,
    }
}

/// Whether the bracketed group at `at` is an attribute: after `#` or `#!`.
fn attribute_at(level: &[TokenTree], at: usize) -> bool {
    let bracketed = matches!(level.get(at), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket);
    let after_hash = punct_at(level, at.wrapping_sub(1), '#')
        || (punct_at(level, at.wrapping_sub(1), '!') && punct_at(level, at.wrapping_sub(2), '#'));
    bracketed && after_hash
}

/// Each module move (`#[path]`) and each call of an `include!` name (`includes`) in `tokens`,
/// macro bodies and function bodies included.
fn inclusions(tokens: TokenStream, includes: &BTreeSet<String>) -> Vec<String> {
    levels(tokens)
        .iter()
        .flat_map(|level| {
            (0..level.len()).filter_map(move |at| {
                if attribute_at(level, at)
                    && let Some(TokenTree::Group(group)) = level.get(at)
                {
                    let meta: Vec<TokenTree> = group.stream().into_iter().collect();
                    return moves_a_module(&meta).then(|| "#[path]".to_owned());
                }
                let ident = ident_at(level, at)?;
                let called = includes.contains(&ident)
                    && punct_at(level, at + 1, '!')
                    && matches!(level.get(at + 2), Some(TokenTree::Group(_)));
                called.then(|| format!("{ident}!"))
            })
        })
        .collect()
}

/// Each module move and `include!` call in `source`, the file `file` (relative to the crates
/// folder), past the [`ALLOWED_INCLUDES`] it holds.
fn inclusion_findings(file: &str, source: &str, includes: &BTreeSet<String>) -> Vec<String> {
    let tokens: TokenStream = source.parse().expect("the source tokenizes");
    let mut left = ALLOWED_INCLUDES
        .iter()
        .find(|(allowed, _)| *allowed == file)
        .map_or(0, |(_, count)| *count);
    let include_call = format!("{INCLUDE}!");
    inclusions(tokens, includes)
        .into_iter()
        .filter(|found| {
            let allowed = *found == include_call && left > 0;
            if allowed {
                left -= 1;
            }
            !allowed
        })
        .map(|found| format!("{file}: {found}"))
        .collect()
}

#[test]
fn no_production_source_moves_a_module_or_includes_one() {
    let includes = with_aliases(&[INCLUDE], &workspace_sources());
    let files: Vec<(String, String)> = workspace_production()
        .iter()
        .map(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            (relative(path, &crates()), source)
        })
        .collect();

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|(file, source)| inclusion_findings(file, source, &includes))
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    for (allowed, count) in ALLOWED_INCLUDES {
        let (_, source) = files
            .iter()
            .find(|(file, _)| file == allowed)
            .unwrap_or_else(|| panic!("the scan reads {allowed}"));
        let tokens: TokenStream = source.parse().expect("the source tokenizes");
        assert_eq!(inclusions(tokens, &includes).len(), count, "{allowed}");
    }
    let ports = "kernel/src/ports.rs";
    let refused = [
        (
            ports,
            "#[path = \"ports/capability/tests.rs\"]\npub mod forge;",
        ),
        (
            ports,
            "#[cfg_attr(not(test), path = \"ports/capability/tests.rs\")]\npub mod forge;",
        ),
        (
            ports,
            "#[cfg_attr(all(), allow(dead_code), cfg_attr(not(test), path = \"forge.rs\"))]\nmod forge;",
        ),
        (
            ports,
            "mod outer {\n    #[path = \"forge.rs\"]\n    mod forge;\n}",
        ),
        (
            ports,
            "fn forge() {\n    #[path = \"forge.rs\"]\n    mod forge;\n}",
        ),
        (
            ports,
            "macro_rules! forge { () => { #[path = \"forge.rs\"] mod forge; }; }",
        ),
        (ports, "include!(\"ports/capability/tests.rs\");"),
        (
            ports,
            "include!(concat!(env!(\"OUT_DIR\"), \"/forge.rs\"));",
        ),
        (ports, "fn forge() { std::include!(\"forge.rs\"); }"),
        (ports, "use std::include as grab;\ngrab!(\"forge.rs\");"),
        (
            "adapter-pg/src/lib.rs",
            "include!(concat!(env!(\"OUT_DIR\"), \"/embedded_migrations.rs\"));\ninclude!(concat!(env!(\"OUT_DIR\"), \"/forge.rs\"));",
        ),
        (
            "api/src/routes/commissions/rename.rs",
            "mod v1 { include!(\"zurfur.api.v1.rs\"); }",
        ),
    ];
    for (file, source) in refused {
        let fixture_includes = with_aliases(&[INCLUDE], &[source.to_owned()]);

        let found = inclusion_findings(file, source, &fixture_includes);

        assert!(!found.is_empty(), "{file}: {source}");
    }
    let allowed = [
        "const LIB: &str = \"mod v1 { include!(\\\"zurfur.api.v1.rs\\\"); }\";",
        "const SQL: &str = include_str!(\"x.sql\");\nconst BYTES: &[u8] = include_bytes!(\"x.bin\");",
        "/// Set `#[path = \"x.rs\"]` nowhere.\nfn path(&self) -> &Path { &self.path }",
        "#[serde(rename = \"path\")]\nstruct Moved { path: String }",
        "#[cfg_attr(path = \"x\", allow(dead_code))]\nfn moved() {}",
        "fn kept(include: bool, other: bool) -> bool { include != other }",
    ];
    for source in allowed {
        let found = inclusion_findings(ports, source, &BTreeSet::from([INCLUDE.to_owned()]));

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

// The write host and the gate.

/// Each place `file` names the write host's constructor (`hosts`) or the gate (`gates`).
fn host_and_gate_namings(
    file_name: &str,
    file: &syn::File,
    hosts: &BTreeSet<String>,
    gates: &BTreeSet<String>,
) -> Vec<Naming> {
    let mut out = Vec::new();
    for piece in pieces(file) {
        let in_host_impl = piece
            .impl_of
            .as_ref()
            .is_some_and(|self_name| hosts.contains(self_name));
        for level in levels(piece.tokens.clone()) {
            for at in 0..level.len() {
                let Some(ident) = ident_at(&level, at) else {
                    continue;
                };
                let names_host = hosts.contains(&ident) || (ident == "Self" && in_host_impl);
                if names_host && path_goes_to(&level, at + 1, "new") {
                    out.push((
                        file_name.to_owned(),
                        piece.context.clone(),
                        format!("{WRITE_HOST}::new"),
                    ));
                }
                let names_gate = gates.contains(&ident)
                    && !defined_at(&level, at)
                    && !path_separator_at(&level, at + 1);
                if names_gate {
                    out.push((file_name.to_owned(), piece.context.clone(), GATE.to_owned()));
                }
            }
        }
    }
    out
}

/// Each naming of the write host's constructor or the gate under `src`, by file relative to it.
fn host_and_gate_namings_under(src: &Path) -> Vec<Naming> {
    let sources: Vec<(PathBuf, String)> = walk(src)
        .production
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            (path, source)
        })
        .collect();
    let texts: Vec<String> = sources.iter().map(|(_, source)| source.clone()).collect();
    let hosts = with_aliases(&[WRITE_HOST], &texts);
    let gates = with_aliases(&[GATE], &texts);
    sources
        .iter()
        .flat_map(|(path, source)| {
            let file = syn::parse_file(source).expect("the source parses");
            host_and_gate_namings(&relative(path, src), &file, &hosts, &gates)
        })
        .collect()
}

/// The two namings allowed: the write host built in [`WRITTEN`], the gate run in [`APPLY`].
fn allowed_host_and_gate() -> Vec<Naming> {
    vec![
        ("den/gate.rs".to_owned(), APPLY.to_owned(), GATE.to_owned()),
        (
            "den/write.rs".to_owned(),
            WRITTEN.to_owned(),
            format!("{WRITE_HOST}::new"),
        ),
    ]
}

/// The namings under `src` past the two allowed, each allowed once.
fn host_and_gate_offenders(src: &Path) -> Vec<Naming> {
    let mut left = allowed_host_and_gate();
    let mut out = Vec::new();
    for naming in host_and_gate_namings_under(src) {
        match left.iter().position(|allowed| *allowed == naming) {
            Some(at) => {
                left.remove(at);
            }
            None => out.push(naming),
        }
    }
    out
}

#[test]
fn the_write_host_and_the_gate_are_named_once() {
    let mut namings = host_and_gate_namings_under(&application_src());
    namings.sort();

    assert_eq!(namings, allowed_host_and_gate());
    let gate_file = "use crate::den::authority::Grant;\npub(in crate::den) struct DenWriteHost<'u> { unit: Unit<'u> }\nimpl<'u> DenWriteHost<'u> { pub(in crate::den) fn new(unit: Unit<'u>) -> Self { Self { unit } } }\nimpl Host for DenWriteHost<'_> { async fn apply(&mut self) { gate(&mut *self.unit).await; } }\npub(in crate::den) async fn gate(unit: Unit<'_>) {}";
    let write_file = "use crate::den::gate::{DenWriteHost};\npub(in crate::den) async fn written(unit: Unit<'_>) { let mut host = DenWriteHost::new(unit); }";
    let allowed = fixture_tree(
        "host_and_gate_allowed",
        &[
            ("lib.rs", "mod den;"),
            ("den.rs", "mod gate;\nmod write;"),
            ("den/gate.rs", gate_file),
            ("den/write.rs", write_file),
        ],
    );
    assert_eq!(host_and_gate_offenders(&allowed), Vec::<Naming>::new());
    let elsewhere = [
        "use crate::den::gate::gate;\nasync fn act(unit: Unit<'_>) { gate(unit).await; }",
        "async fn act(unit: Unit<'_>) { crate::den::gate::gate(unit).await; }",
        "use crate::den::gate::gate as run;\nasync fn act(unit: Unit<'_>) { run(unit).await; }",
        "use crate::den::gate::DenWriteHost;\nfn act(unit: Unit<'_>) { let host = DenWriteHost::new(unit); }",
        "use crate::den::gate::DenWriteHost as Host;\nfn act(unit: Unit<'_>) { let host = Host::new(unit); }",
        "fn act(unit: Unit<'_>) { let host = crate::den::gate::DenWriteHost::<'_>::new(unit); }",
        "fn act(units: Vec<Unit<'_>>) { units.into_iter().map(<crate::den::gate::DenWriteHost>::new); }",
        "fn act(unit: Unit<'_>) { tokio::join!(async { crate::den::gate::gate(unit).await }); }",
    ];
    for (index, source) in elsewhere.into_iter().enumerate() {
        let src = fixture_tree(
            &format!("host_and_gate_elsewhere_{index}"),
            &[
                ("lib.rs", "mod commission;\nmod den;"),
                ("commission.rs", "mod act;"),
                ("commission/act.rs", source),
                ("den.rs", "mod gate;\nmod write;"),
                ("den/gate.rs", gate_file),
                ("den/write.rs", write_file),
            ],
        );

        let offenders = host_and_gate_offenders(&src);

        assert!(!offenders.is_empty(), "{source}");
    }
    let twice_in_the_gate_file = gate_file.replace(
        "pub(in crate::den) async fn gate(unit: Unit<'_>) {}",
        "pub(in crate::den) async fn gate(unit: Unit<'_>) {}\nimpl<'u> DenWriteHost<'u> { fn again(unit: Unit<'u>) -> Self { let other = gate; Self::new(unit) } }",
    );
    let src = fixture_tree(
        "host_and_gate_twice",
        &[
            ("lib.rs", "mod den;"),
            ("den.rs", "mod gate;\nmod write;"),
            ("den/gate.rs", &twice_in_the_gate_file),
            ("den/write.rs", write_file),
        ],
    );
    assert_eq!(host_and_gate_offenders(&src).len(), 2);
}

// The write use case stays inside the crate.

/// Whether `vis` makes an item visible outside its crate.
fn public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

#[test]
fn the_write_use_case_stays_crate_private() {
    let write = parse(&application_src().join("den/write.rs"));
    let act = parse(&application_src().join("den/act.rs"));

    let write_method: Vec<bool> = write
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(implementation) if type_name(&implementation.self_ty) == "Den" => {
                Some(implementation)
            }
            _ => None,
        })
        .flat_map(|implementation| &implementation.items)
        .filter_map(|impl_item| match impl_item {
            ImplItem::Fn(method) if method.sig.ident == "write" => Some(public(&method.vis)),
            _ => None,
        })
        .collect();
    let command: Vec<bool> = write
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(structure) if structure.ident == "Command" => Some(public(&structure.vis)),
            _ => None,
        })
        .collect();
    let den_act: Vec<bool> = act
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(enumeration) if enumeration.ident == "DenAct" => {
                Some(public(&enumeration.vis))
            }
            _ => None,
        })
        .collect();

    assert_eq!(write_method, [false], "Den::write");
    assert_eq!(command, [false], "Command");
    assert_eq!(den_act, [false], "DenAct");
}

// Nothing in the drivers.

/// The in-memory adapter's commission seeders, each a write beneath the gate: a bound
/// commission, a stored title past the name rule, a lifecycle step, a tab row, and standing
/// given or taken. The dev profile reaches them only through the fixture.
const IN_MEMORY_SEEDERS: [&str; 6] = [
    "seed_bound_commission",
    "seed_stored_title",
    "seed_lifecycle_step",
    "seed_tab",
    "seed_participant",
    "remove_participant",
];

/// What a driver may not name: the node port, the issuer, the capability, the in-memory
/// seeders, and raw SQL.
fn driver_forbidden(sources: &[String]) -> BTreeSet<String> {
    let seeds: Vec<&str> = NODE_PORT
        .iter()
        .copied()
        .chain([GATE_ISSUED, WRITE_CAPABILITY])
        .chain(IN_MEMORY_SEEDERS)
        .chain(RAW_SQL)
        .collect();
    with_aliases(&seeds, sources)
}

/// Each call or path naming the unit's commission repo or changelog in `level`.
fn repo_calls(level: &[TokenTree]) -> Vec<String> {
    (0..level.len())
        .filter_map(|at| {
            let ident = ident_at(level, at)?;
            if !COMMISSION_REPOS.contains(&ident.as_str()) || defined_at(level, at) {
                return None;
            }
            let called = parenthesized_at(level, at + 1) || path_separator_at(level, at + 1);
            let pathed = at >= 2 && path_separator_at(level, at - 2);
            (called || pathed).then_some(ident)
        })
        .collect()
}

/// Whether the attribute group at `at` (after `#` or `#!`) is a doc comment.
fn doc_attribute_at(level: &[TokenTree], at: usize) -> bool {
    let Some(TokenTree::Group(group)) = level.get(at) else {
        return false;
    };
    let after_hash = punct_at(level, at.wrapping_sub(1), '#')
        || (punct_at(level, at.wrapping_sub(1), '!') && punct_at(level, at.wrapping_sub(2), '#'));
    let first = group.stream().into_iter().next();
    let is_doc = matches!(first, Some(TokenTree::Ident(ident)) if ident == "doc");
    group.delimiter() == Delimiter::Bracket && after_hash && is_doc
}

/// The text of a string literal token, or `None` for any other literal.
fn literal_text(tree: &TokenTree) -> Option<String> {
    let TokenTree::Literal(literal) = tree else {
        return None;
    };
    match syn::parse_str::<syn::Lit>(&literal.to_string()).ok()? {
        syn::Lit::Str(text) => Some(text.value()),
        syn::Lit::ByteStr(bytes) => Some(String::from_utf8_lossy(&bytes.value()).into_owned()),
        syn::Lit::CStr(text) => Some(text.value().to_string_lossy().into_owned()),
        _ => None,
    }
}

/// Every string literal in `tokens` outside doc comments, and each `concat!`'s pieces joined.
fn string_literals(tokens: TokenStream) -> Vec<String> {
    let level: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out = Vec::new();
    for (at, tree) in level.iter().enumerate() {
        if let Some(text) = literal_text(tree) {
            out.push(text);
        }
        let TokenTree::Group(group) = tree else {
            continue;
        };
        if doc_attribute_at(&level, at) {
            continue;
        }
        let concatenated = punct_at(&level, at.wrapping_sub(1), '!')
            && ident_at(&level, at.wrapping_sub(2)).as_deref() == Some("concat");
        if concatenated {
            let joined: String = group
                .stream()
                .into_iter()
                .filter_map(|piece| literal_text(&piece))
                .collect();
            out.push(joined);
        }
        out.extend(string_literals(group.stream()));
    }
    out
}

/// `sql` without its comments: `--` to the line's end and `/* */`, nested ones included.
fn without_sql_comments(sql: &str) -> String {
    let characters: Vec<char> = sql.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    let mut depth = 0usize;
    while at < characters.len() {
        let pair = (characters[at], characters.get(at + 1).copied());
        if pair == ('/', Some('*')) {
            depth += 1;
            at += 2;
            out.push(' ');
        } else if depth > 0 && pair == ('*', Some('/')) {
            depth -= 1;
            at += 2;
        } else if depth > 0 {
            at += 1;
        } else if pair == ('-', Some('-')) {
            while at < characters.len() && characters[at] != '\n' {
                at += 1;
            }
        } else {
            out.push(characters[at]);
            at += 1;
        }
    }
    out
}

/// A word of SQL: a bare word, a quoted identifier, or one other character.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SqlWord {
    Bare(String),
    Quoted(String),
    Other(char),
}

fn sql_words(sql: &str) -> Vec<SqlWord> {
    let characters: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < characters.len() {
        let character = characters[at];
        if character.is_whitespace() {
            at += 1;
        } else if character.is_alphanumeric() || character == '_' {
            let start = at;
            while at < characters.len()
                && (characters[at].is_alphanumeric()
                    || characters[at] == '_'
                    || characters[at] == '$')
            {
                at += 1;
            }
            out.push(SqlWord::Bare(characters[start..at].iter().collect()));
        } else if character == '"' || character == '\'' {
            let mut text = String::new();
            at += 1;
            while at < characters.len() {
                if characters[at] == character {
                    if characters.get(at + 1) == Some(&character) {
                        text.push(character);
                        at += 2;
                        continue;
                    }
                    break;
                }
                text.push(characters[at]);
                at += 1;
            }
            at += 1;
            if character == '"' {
                out.push(SqlWord::Quoted(text));
            } else {
                out.push(SqlWord::Other('\''));
            }
        } else {
            out.push(SqlWord::Other(character));
            at += 1;
        }
    }
    out
}

fn is_node(word: Option<&SqlWord>) -> bool {
    match word {
        Some(SqlWord::Bare(bare)) => bare.eq_ignore_ascii_case(NODE_TABLE),
        Some(SqlWord::Quoted(quoted)) => quoted == NODE_TABLE,
        _ => false,
    }
}

fn is_bare(word: Option<&SqlWord>, expected: &str) -> bool {
    matches!(word, Some(SqlWord::Bare(bare)) if bare.eq_ignore_ascii_case(expected))
}

/// Whether the table named at `at` is the node table: `node`, `"node"`, or `<schema>.node`.
fn names_node_at(words: &[SqlWord], at: usize) -> bool {
    let qualified = matches!(words.get(at), Some(SqlWord::Bare(_) | SqlWord::Quoted(_)))
        && words.get(at + 1) == Some(&SqlWord::Other('.'))
        && is_node(words.get(at + 2));
    is_node(words.get(at)) || qualified
}

/// Whether `sql` names the node table after a word a table follows, or in a list after a
/// comma; `ONLY` may stand between.
fn names_the_node_table(sql: &str) -> bool {
    let words = sql_words(&without_sql_comments(sql));
    (0..words.len()).any(|at| {
        let introduces = SQL_KEYWORDS
            .iter()
            .any(|keyword| is_bare(words.get(at), keyword))
            || words.get(at) == Some(&SqlWord::Other(','));
        if !introduces {
            return false;
        }
        let table = if is_bare(words.get(at + 1), "ONLY") {
            at + 2
        } else {
            at + 1
        };
        names_node_at(&words, table)
    })
}

/// Whether `sql`, its comments stripped, holds a Unicode-escaped quoted identifier.
fn holds_a_unicode_identifier(sql: &str) -> bool {
    without_sql_comments(sql)
        .to_ascii_lowercase()
        .contains(UNICODE_IDENTIFIER)
}

/// Each string literal in `tokens` that names the node table or a `.sql` file, or holds a
/// Unicode-escaped identifier.
fn sql_findings(tokens: TokenStream) -> Vec<String> {
    string_literals(tokens)
        .into_iter()
        .filter(|text| {
            names_the_node_table(text)
                || holds_a_unicode_identifier(text)
                || text.to_ascii_lowercase().ends_with(".sql")
        })
        .collect()
}

/// What breaks the driver rules in `source`, the file `file` of crate `krate`.
fn driver_findings(
    krate: &str,
    file: &str,
    source: &str,
    forbidden: &BTreeSet<String>,
) -> Vec<String> {
    let parsed = syn::parse_file(source).expect("the source parses");
    let pieces = pieces(&parsed);
    let allowance: Vec<(&str, &str, usize)> = if file == SWEEP {
        vec![(LEADER_LOCK, "sqlx", 1)]
    } else {
        Vec::new()
    };
    let mut out = show(&named_past(file, &pieces, forbidden, &allowance));
    for piece in &pieces {
        if krate != API {
            for level in levels(piece.tokens.clone()) {
                for call in repo_calls(&level) {
                    out.push(format!("{file}: `{call}` called in {}", piece.context));
                }
            }
        }
        for text in sql_findings(piece.tokens.clone()) {
            out.push(format!("{file}: SQL {text:?} in {}", piece.context));
        }
    }
    out
}

/// Each driver crate's production files, with the crate's name and each file's path from the
/// crates folder.
fn driver_files() -> Vec<(String, String, PathBuf)> {
    drivers()
        .iter()
        .flat_map(|member| {
            walk(&member.folder.join("src"))
                .production
                .into_iter()
                .map(|path| (member.name.clone(), relative(&path, &crates()), path))
        })
        .collect()
}

#[test]
fn no_driver_holds_the_node_port_raw_sql_or_a_capability() {
    let forbidden = driver_forbidden(&workspace_sources());
    let files = driver_files();

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|(krate, file, path)| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            driver_findings(krate, file, &source, &forbidden)
        })
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let sweep = files.iter().any(|(_, file, _)| file == SWEEP);
    assert!(sweep, "the scan reads {SWEEP}");
}

#[test]
fn the_driver_scan_refuses_each_planted_route() {
    let forbidden = driver_forbidden(&[]);
    let route = "api/src/routes/commissions/rename.rs";
    let planted = [
        "async fn rename(state: AppState) { state.transaction(async move |uow: &mut dyn UnitOfWork| { uow.nodes().set_name(&id, &name).await }).await; }",
        "fn rename(id: CommissionId) { let capability = WriteCapability::<_, AsOwner>::gate_issued(id); }",
        "fn rename(capability: &domain::ports::WriteCapability<CommissionId, AsOwner>) {}",
        "use domain::ports::WriteCapability as Leave;",
        "async fn seed(backend: MemBackend) { let store = backend.node_store(); }",
        "async fn seed(backend: MemBackend, seed: CommissionSeed) { backend.seed_bound_commission(&seed).await; }",
        "use adapter_mem::MemBackend;\nfn seeder() -> fn(&MemBackend) { MemBackend::seed_bound_commission }",
        "fn seed(backend: &MemBackend) { backend.seed_stored_title(adapter_mem::fixture::untitled(), \"a/name\"); }",
        "fn seed(backend: &MemBackend) { adapter_mem::MemBackend::seed_lifecycle_step(backend, untitled(), LifecycleStep::Active); }",
        "use adapter_mem::MemBackend;\nfn seed(backend: &MemBackend, user: UserId) { backend.seed_participant(untitled(), user, now()); }",
        "fn seeder() -> fn(&MemBackend, CommissionId, &UserId) { MemBackend::remove_participant }",
        "fn seed(backend: &MemBackend, tab: TabName) { let tab_id = backend.seed_tab(untitled(), tab); }",
        "async fn rename(state: AppState) { adapter_pg::queries::node::set_name(&state.pool, &id, &name).await; }",
        "use adapter_pg::queries as q;",
        "use adapter_pg::queries::commission::add_participant;",
        "async fn rename(state: AppState, table: &str) { sqlx::query(&format!(\"UPDATE {table} SET name = $1\")).execute(&state.pool).await; }",
        "async fn rename(state: AppState) { sqlx::raw_sql(include_str!(\"x.sql\")).execute(&state.pool).await; }",
        "async fn rename(state: AppState) { run(&state.pool, include_str!(\"rename.sql\")).await; }",
        "const RENAME: &str = \"UPDATE node SET name = $1\";",
        "const RENAME: &str = \"UPDATE \\\"node\\\" SET name = $1\";",
        "const RENAME: &str = \"UPDATE public.node SET name = $1\";",
        "const RENAME: &str = \"update only node set name = $1\";",
        "const RENAME: &str = \"TRUNCATE node\";",
        "const RENAME: &str = \"UPDATE /* the den */ node SET name = $1\";",
        "const RENAME: &str = \"UPDATE -- the den\\n \\\"node\\\" SET name = $1\";",
        "const RENAME: &str = \"UPDATE public /* the den */ . node SET name = $1\";",
        "const RENAME: &str = \"UPDATE ONLY /* the den */ node SET name = $1\";",
        "const RENAME: &str = \"TRUNCATE -- the den\\n node\";",
        "const RENAME: &str = \"DELETE FROM commission c USING account a, node n WHERE true\";",
        "const RENAME: &str = concat!(\"UPDATE \", \"node SET name = $1\");",
        "const RENAME: &[u8] = b\"INSERT INTO node VALUES ($1)\";",
        "const RENAME: &str = r#\"MERGE INTO \"node\" USING x ON true\"#;",
        "const RENAME: &str = \"UPDATE U&\\\"\\\\006Eode\\\" SET name = $1\";",
        "const RENAME: &str = \"update u&\\\"!006Eode\\\" uescape '!' set name = $1\";",
        "const RENAME: &str = \"CREATE TRIGGER rename BEFORE INSERT ON node FOR EACH ROW EXECUTE FUNCTION f()\";",
    ];
    for source in planted {
        let found = driver_findings(API, route, source, &forbidden);

        assert!(!found.is_empty(), "{source}");
    }
    let repo_calls = [
        "async fn set(uow: &mut dyn UnitOfWork) { uow.commissions().set_deadline(&id, None).await; }",
        "async fn set(uow: &mut dyn UnitOfWork) { uow.changelog().append(&entry).await; }",
        "async fn set(uow: &mut dyn UnitOfWork) { UnitOfWork::commissions(uow).set_deadline(&id, None).await; }",
        "fn set(units: Vec<Box<dyn UnitOfWork>>) { units.into_iter().map(<dyn UnitOfWork>::changelog); }",
        "async fn set(uow: &mut dyn UnitOfWork) { tokio::join!(async { uow.commissions().delete(&id).await }); }",
    ];
    for source in repo_calls {
        for (krate, file) in [
            ("cli", "cli/src/commands/commission.rs"),
            ("composition", "composition/src/runtime.rs"),
            ("worker", "worker/src/sweep.rs"),
        ] {
            let found = driver_findings(krate, file, source, &forbidden);

            assert!(!found.is_empty(), "{krate}: {source}");
        }
        assert_eq!(
            driver_findings(API, route, source, &forbidden),
            Vec::<String>::new(),
            "clippy holds api's: {source}"
        );
    }
    let allowed = [
        "/// Reads the rows from the node table, then updates node names.\nfn read() {}",
        "const READ: &str = \"SELECT id FROM commission JOIN node_closure c ON true\";",
        "const READ: &str = \"the node's name\";",
        "const READ: &str = \"UPDATE \\\"Node\\\" SET x = 1\";",
        "const READ: &str = \"SELECT U&'d\\\\0061ta' FROM commission\";",
        "const READ: &str = \"SELECT 1 FROM commission -- not U&\\\"node\\\"\";",
        "const READ: &str = \"CREATE INDEX ON commission (account)\";",
        "struct Ports { commissions: Arc<dyn CommissionStore>, changelog: Arc<dyn ChangelogStore> }",
        "fn ports(runtime: &Runtime) { let store = runtime.ports.commissions.clone(); }",
    ];
    for source in allowed {
        let found = driver_findings(
            "composition",
            "composition/src/ports.rs",
            source,
            &forbidden,
        );

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

#[test]
fn the_sweep_exemption_holds_only_inside_its_one_function() {
    let forbidden = driver_forbidden(&[]);
    let leader = "async fn sweep_pass_as_leader(pool: &PgPool) -> anyhow::Result<bool> { let mut guard = pool.begin().await?; let acquired: bool = sqlx::query_scalar(\"SELECT pg_try_advisory_xact_lock($1)\").bind(KEY).fetch_one(&mut *guard).await?; Ok(acquired) }";
    let elsewhere = [
        "async fn run_deadline_sweeper(pool: PgPool) { sqlx::query(\"SELECT 1\").execute(&pool).await; }",
        "use sqlx::Executor;",
        "async fn sweep_pass_as_leader(pool: &PgPool) { sqlx::query(\"SELECT 1\").execute(pool).await; sqlx::query(\"SELECT 2\").execute(pool).await; }",
        "mod inner { async fn sweep_pass_as_leader(pool: &PgPool) { sqlx::query(\"SELECT 1\").execute(pool).await; } }",
        "async fn sweep_pass_as_leader(pool: &PgPool) { adapter_pg::queries::commission::mark_late(pool).await; }",
    ];

    let allowed = driver_findings(API, SWEEP, leader, &forbidden);

    assert_eq!(allowed, Vec::<String>::new());
    assert!(!driver_findings(API, "api/src/main.rs", leader, &forbidden).is_empty());
    for source in elsewhere {
        let found = driver_findings(API, SWEEP, source, &forbidden);

        assert!(!found.is_empty(), "{source}");
    }
}

#[test]
fn the_scan_reads_every_crate_that_depends_on_composition() {
    let raw = metadata();
    let members = members_of(&raw);

    let scanned: BTreeSet<String> = drivers().into_iter().map(|member| member.name).collect();

    let packages = raw["packages"].as_array().expect("a package list");
    let depends_on_any = |package: &serde_json::Value, drivers: &BTreeSet<String>| {
        let dependencies = package["dependencies"]
            .as_array()
            .expect("a dependency list");
        dependencies.iter().any(|dependency| {
            let on_a_driver = dependency["name"]
                .as_str()
                .is_some_and(|name| drivers.contains(name));
            on_a_driver && dependency["kind"].is_null()
        })
    };
    let mut expected: BTreeSet<String> = BTreeSet::from([seams::COMPOSITION.to_owned()]);
    loop {
        let reached: Vec<String> = packages
            .iter()
            .filter(|package| depends_on_any(package, &expected))
            .filter_map(|package| package["name"].as_str())
            .filter(|name| *name != seams::TEST_SUPPORT && !expected.contains(*name))
            .map(str::to_owned)
            .collect();
        if reached.is_empty() {
            break;
        }
        expected.extend(reached);
    }
    for driver in ["api", "cli", "composition"] {
        assert!(scanned.contains(driver), "{driver} is scanned: {scanned:?}");
    }
    assert!(expected.is_subset(&scanned), "{expected:?} ⊄ {scanned:?}");
    assert!(!scanned.contains(seams::APPLICATION));
    let test_support = members
        .iter()
        .find(|member| member.name == seams::TEST_SUPPORT)
        .expect("the test rig is a member");
    assert!(test_support.depends_normally_on(seams::COMPOSITION));
    let built_on_the_rig: Vec<&str> = members
        .iter()
        .filter(|member| member.depends_normally_on(seams::TEST_SUPPORT))
        .map(|member| member.name.as_str())
        .collect();
    assert_eq!(
        built_on_the_rig,
        Vec::<&str>::new(),
        "the test rig is excluded from the drivers only while every crate takes it as a dev-dependency"
    );
    for (_, _, path) in driver_files() {
        assert!(path.is_file());
    }
    let synthetic = serde_json::json!({ "packages": [
        { "name": "composition", "manifest_path": "/w/composition/Cargo.toml", "dependencies": [] },
        { "name": "worker", "manifest_path": "/w/worker/Cargo.toml", "dependencies": [
            { "name": "composition", "kind": null }
        ] },
        { "name": "builder", "manifest_path": "/w/builder/Cargo.toml", "dependencies": [
            { "name": "composition", "kind": "build" }
        ] },
        { "name": "bench", "manifest_path": "/w/bench/Cargo.toml", "dependencies": [
            { "name": "composition", "kind": "dev" }
        ] },
        { "name": "test-support", "manifest_path": "/w/test-support/Cargo.toml", "dependencies": [
            { "name": "composition", "kind": null }
        ] },
        { "name": "jobs", "manifest_path": "/w/jobs/Cargo.toml", "dependencies": [
            { "name": "api", "kind": null }
        ] },
        { "name": "api", "manifest_path": "/w/api/Cargo.toml", "dependencies": [
            { "name": "composition", "kind": null }
        ] },
        { "name": "docs", "manifest_path": "/w/docs/Cargo.toml", "dependencies": [
            { "name": "jobs", "kind": "dev" }
        ] }
    ] });
    let synthetic_drivers: Vec<String> = drivers_of(&members_of(&synthetic))
        .into_iter()
        .map(|member| member.name)
        .collect();
    assert_eq!(
        synthetic_drivers,
        ["composition", "worker", "builder", "jobs", "api"]
    );
}

// No lint silencer in any production file of the use-case layer.

/// The lints no production file of the use-case layer may silence: an item no real act
/// reaches is removed, never silenced. `warnings` silences them all.
const SILENCED_LINTS: [&str; 5] = [
    "dead_code",
    "unused",
    "unused_variables",
    "unreachable_code",
    "warnings",
];

/// The lint attributes that silence a lint.
const SILENCERS: [&str; 2] = ["allow", "expect"];

/// The attribute that applies others under a condition.
const CFG_ATTR: &str = "cfg_attr";

/// What the silencer scan of one source tree read and found.
struct SilencerScan {
    /// Each file read, relative to the tree's `src`.
    read: Vec<String>,
    /// Each silencer found, as `file: silencer`.
    found: Vec<String>,
}

/// Reads every production file under `src`, the crate root and every module file included,
/// for a silencer: nothing depends on how a file's code is written or which files hold acts.
fn silencer_scan(src: &Path) -> SilencerScan {
    let mut scan = SilencerScan {
        read: Vec::new(),
        found: Vec::new(),
    };
    for path in walk(src).production {
        let file = relative(&path, src);
        let source = std::fs::read_to_string(&path).expect("a readable source");
        scan.found.extend(silencer_findings(&file, &source));
        scan.read.push(file);
    }
    scan
}

/// An identifier's text, a raw identifier's `r#` dropped.
fn unraw(ident: &proc_macro2::Ident) -> String {
    ident.unraw().to_string()
}

/// `stream` cut at its top-level commas.
fn comma_separated(stream: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut parts = vec![Vec::new()];
    for tree in stream {
        let comma = matches!(&tree, TokenTree::Punct(punct) if punct.as_char() == ',');
        if comma {
            parts.push(Vec::new());
            continue;
        }
        if let Some(part) = parts.last_mut() {
            part.push(tree);
        }
    }
    parts.retain(|part| !part.is_empty());
    parts
}

/// The silenced lints one attribute's contents name: an `allow` or `expect` of a lint in
/// [`SILENCED_LINTS`] (any letter case), bare or under any depth of `cfg_attr`.
fn silenced_by(meta: &[TokenTree]) -> Vec<String> {
    let (Some(TokenTree::Ident(name)), Some(TokenTree::Group(arguments))) =
        (meta.first(), meta.get(1))
    else {
        return Vec::new();
    };
    if arguments.delimiter() != Delimiter::Parenthesis {
        return Vec::new();
    }
    let name = unraw(name);
    let parts = comma_separated(arguments.stream());
    if name == CFG_ATTR {
        return parts
            .iter()
            .skip(1)
            .flat_map(|applied| silenced_by(applied))
            .collect();
    }
    if !SILENCERS.contains(&name.as_str()) {
        return Vec::new();
    }
    parts
        .iter()
        .filter_map(|lint| match lint.as_slice() {
            [TokenTree::Ident(lint)] => Some(unraw(lint).to_lowercase()),
            _ => None,
        })
        .filter(|lint| SILENCED_LINTS.contains(&lint.as_str()))
        .map(|lint| format!("{name}({lint})"))
        .collect()
}

/// Every silencer in `tokens`, at any depth: each outer `#[…]` or inner `#![…]` attribute,
/// in items, bodies and macro bodies alike, that silences a lint in [`SILENCED_LINTS`].
fn silencers(tokens: TokenStream) -> Vec<String> {
    let level: Vec<TokenTree> = tokens.into_iter().collect();
    let mut found = Vec::new();
    for (at, tree) in level.iter().enumerate() {
        if let TokenTree::Group(group) = tree {
            found.extend(silencers(group.stream()));
        }
        if !punct_at(&level, at, '#') {
            continue;
        }
        let bracket_at = if punct_at(&level, at + 1, '!') {
            at + 2
        } else {
            at + 1
        };
        let Some(TokenTree::Group(attribute)) = level.get(bracket_at) else {
            continue;
        };
        if attribute.delimiter() != Delimiter::Bracket {
            continue;
        }
        let meta: Vec<TokenTree> = attribute.stream().into_iter().collect();
        found.extend(silenced_by(&meta));
    }
    found
}

/// Each silencer in `source`, from `file`.
fn silencer_findings(file: &str, source: &str) -> Vec<String> {
    let tokens: TokenStream = source.parse().expect("the source lexes");
    silencers(tokens)
        .into_iter()
        .map(|silencer| format!("{file}: {silencer}"))
        .collect()
}

#[test]
fn no_production_source_holds_a_lint_silencer() {
    let src = application_src();

    let scan = silencer_scan(&src);

    assert_eq!(scan.found, Vec::<String>::new());
    let production: Vec<String> = walk(&src)
        .production
        .iter()
        .map(|path| relative(path, &src))
        .collect();
    assert_eq!(
        scan.read, production,
        "the scan reads every production file"
    );
    for file in [
        "lib.rs",
        "commission.rs",
        "den.rs",
        GATE_FILE,
        "den/act.rs",
        "den/write.rs",
        "den/authority.rs",
        "commission/act.rs",
        "commission/create.rs",
        "commission/rename.rs",
        "commission/archive.rs",
        "commission/den.rs",
    ] {
        assert!(
            scan.read.iter().any(|read| read == file),
            "the scan reads {file}: {:?}",
            scan.read
        );
    }
}

#[test]
fn a_silencer_is_found_in_any_production_file_however_its_acts_are_written() {
    let library = "#[allow(dead_code)]\nmod bystander;\nmod commission;\nmod den;\n#[cfg(test)]\nmod test_world;\n";
    let stamping = "macro_rules! read_back_handler {\n    ($act:ident) => {\n        impl crate::den::Handler for $act {}\n    };\n}\n\npub(crate) use read_back_handler;\n";
    let stamped = "pub(crate) struct Archive;\n\ncrate::commission::act::read_back_handler!(Archive);\n\n#[allow(dead_code)]\nfn never_back_to_draft() -> bool {\n    false\n}\n";
    let blanket =
        "pub(crate) trait FieldAct {}\n\nimpl<T: FieldAct> crate::den::Handler for T {}\n";
    let field_act = "pub(crate) struct Rename;\n\nimpl crate::commission::field::FieldAct for Rename {}\n\n#[expect(unused_variables)]\nfn later(name: &str) {}\n";
    let in_a_body = "pub(crate) struct Planted;\n\nfn hidden() {\n    use crate::den::Handler as Deed;\n    impl Deed for Planted {}\n    #[allow(unreachable_code)]\n    let never = 1;\n}\n";
    let bystander = "#![allow(warnings)]\n\nfn untouched() {}\n";
    let test_world = "#[allow(dead_code)]\nfn only_a_test_builds_it() {}\n";
    let src = fixture_tree(
        "silencer_scan",
        &[
            ("lib.rs", library),
            ("den.rs", "pub(crate) trait Handler {}\n"),
            (
                "commission.rs",
                "mod act;\nmod archive;\nmod deed;\nmod field;\nmod rename;\n",
            ),
            ("commission/act.rs", stamping),
            ("commission/archive.rs", stamped),
            ("commission/field.rs", blanket),
            ("commission/rename.rs", field_act),
            ("commission/deed.rs", in_a_body),
            ("bystander.rs", bystander),
            ("test_world.rs", test_world),
        ],
    );

    let scan = silencer_scan(&src);

    let mut found = scan.found;
    found.sort();
    let expected_found = [
        "bystander.rs: allow(warnings)",
        "commission/archive.rs: allow(dead_code)",
        "commission/deed.rs: allow(unreachable_code)",
        "commission/rename.rs: expect(unused_variables)",
        "lib.rs: allow(dead_code)",
    ];
    assert_eq!(found, expected_found);
    let mut read = scan.read;
    read.sort();
    let expected_read = [
        "bystander.rs",
        "commission.rs",
        "commission/act.rs",
        "commission/archive.rs",
        "commission/deed.rs",
        "commission/field.rs",
        "commission/rename.rs",
        "den.rs",
        "lib.rs",
    ];
    assert_eq!(read, expected_read, "a test-only module is not production");
}

#[test]
fn the_silencer_parser_finds_every_spelling() {
    let sweep = "#[allow(\n    clippy::disallowed_methods,\n    reason = \"the sweep is a system job, not a use-case method\"\n)]\npub async fn sweep_deadlines() {}";
    let refused = [
        "#[allow(dead_code)]\nfn grant_fits() {}",
        "#[expect(dead_code, reason = \"only a test builds it\")]\nenum Anchor { Thing }",
        "#![allow(unused)]\nfn later() {}",
        "#![expect(unused_variables, reason = \"later\")]\nfn later(grant: Grant) {}",
        "#[allow(unreachable_code)]\nfn later() { return; }",
        "#[allow(warnings)]\nfn later() {}",
        "#[cfg_attr(not(test), expect(dead_code, reason = \"only a test builds one\"))]\nenum Anchor { Thing }",
        "#[cfg_attr(test, allow(unused_variables))]\nfn later(grant: Grant) {}",
        "#![cfg_attr(not(test), allow(dead_code))]\nfn later() {}",
        "#[cfg_attr(feature = \"x\", cfg_attr(not(test), allow(dead_code)))]\nfn later() {}",
        "#[allow(clippy::disallowed_methods, dead_code)]\nfn later() {}",
        "#[allow(Dead_Code)]\nfn later() {}",
        "#[r#allow(dead_code)]\nfn later() {}",
        "fn later() { #[allow(unused_variables)] let grant = 1; }",
        "impl Grant { #[expect(dead_code)] fn later(&self) {} }",
        "macro_rules! later { () => { #[allow(dead_code)] fn later() {} }; }",
        "#![allow(dead_code)]\npub mod den;",
        "#[allow(dead_code)]\npub mod den;",
        "#[allow(dead_code)]\nmod values;",
        "#[expect(unused, reason = \"later\")]\npub mod commission;",
        "#![cfg_attr(not(test), allow(warnings))]\npub mod account;",
        "#[cfg_attr(not(test), allow(dead_code))]\npub(crate) mod act;",
        "#[allow(Dead_Code)]\npub(crate) mod r#den;",
        "const _: () = {\n    use crate::den::Handler as Deed;\n    #[allow(dead_code)]\n    fn later() {}\n};",
    ];
    for source in refused {
        let found = silencer_findings("den/gate.rs", source);

        assert_eq!(found.len(), 1, "{source}: {found:?}");
    }
    let kept = [
        "#[allow(clippy::disallowed_methods, reason = \"a test opens its unit\")]\nfn opens() {}",
        "#[expect(clippy::disallowed_methods, reason = \"the gate's one node port\")]\nfn gate() {}",
        sweep,
        "#[cfg_attr(test, derive(Debug))]\nstruct Grant;",
        "#[cfg_attr(test, derive(Debug))]\npub mod den;",
        "/// Never `#[allow(dead_code)]` here.\nfn documented() {}",
        "//! Never `#![allow(dead_code)]` here.\n\npub mod den;",
        "fn named() { let allow = \"dead_code\"; }",
        "#[cfg(test)]\nmod tests;",
    ];
    for source in kept {
        let found = silencer_findings("den/gate.rs", source);

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

// The interim guard: the frozen list.

/// Each commission field write in `file`: a [`FIELD_WRITES`] name anywhere, and a
/// [`REPO_WRITES`] name on a `commissions()` receiver or on a local bound from one.
fn field_writes(file_name: &str, file: &syn::File) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for piece in pieces(file) {
        let piece_levels = levels(piece.tokens.clone());
        let bound: BTreeSet<String> = piece_levels
            .iter()
            .flat_map(|level| bound_from_commissions(level))
            .collect();
        for level in &piece_levels {
            for at in 0..level.len() {
                let Some(ident) = ident_at(level, at) else {
                    continue;
                };
                let by_name = FIELD_WRITES.contains(&ident.as_str()) && !defined_at(level, at);
                let on_the_repo = REPO_WRITES.contains(&ident.as_str())
                    && at >= 2
                    && punct_at(level, at - 1, '.')
                    && receiver_is_the_repo(level, at - 2, &bound);
                if by_name || on_the_repo {
                    out.push((file_name.to_owned(), piece.context.clone(), ident));
                }
            }
        }
    }
    out
}

/// The locals `level` binds from a `commissions(…)` call: `let [mut] x = ….commissions(…);`.
fn bound_from_commissions(level: &[TokenTree]) -> Vec<String> {
    (0..level.len())
        .filter(|at| ident_at(level, *at).as_deref() == Some("let"))
        .filter_map(|at| {
            let name_at = if ident_at(level, at + 1).as_deref() == Some("mut") {
                at + 2
            } else {
                at + 1
            };
            let name = ident_at(level, name_at)?;
            let end = (name_at..level.len()).find(|index| punct_at(level, *index, ';'))?;
            let from_commissions = end >= 2
                && parenthesized_at(level, end - 1)
                && ident_at(level, end - 2).as_deref() == Some("commissions");
            from_commissions.then_some(name)
        })
        .collect()
}

/// Whether the receiver ending at `at` is a commission repo: a `commissions(…)` call, a local
/// bound from one, or a parenthesised expression naming either.
fn receiver_is_the_repo(level: &[TokenTree], at: usize, bound: &BTreeSet<String>) -> bool {
    if let Some(name) = ident_at(level, at) {
        return bound.contains(&name);
    }
    let Some(TokenTree::Group(group)) = level.get(at) else {
        return false;
    };
    if group.delimiter() != Delimiter::Parenthesis {
        return false;
    }
    let called = at >= 1 && ident_at(level, at - 1).as_deref() == Some("commissions");
    let wraps = identifiers(group.stream())
        .iter()
        .any(|ident| ident == "commissions" || bound.contains(ident));
    called || wraps
}

/// Each commission field write in the production sources under `src`, sorted.
fn field_writes_under(src: &Path) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = walk(src)
        .production
        .iter()
        .flat_map(|path| field_writes(&relative(path, src), &parse(path)))
        .collect();
    out.sort();
    out
}

#[test]
fn the_ungated_field_writes_are_exactly_these() {
    let found = field_writes_under(&application_src());

    let frozen: Vec<(String, String, String)> = FROZEN_FIELD_WRITES
        .iter()
        .map(|(file, function, method)| {
            (
                (*file).to_owned(),
                (*function).to_owned(),
                (*method).to_owned(),
            )
        })
        .collect();
    assert_eq!(found, frozen);
    let new_sites = [
        "impl Deadline<'_> { async fn extend(&self, uow: Unit<'_>) { uow.commissions().set_deadline(&id, Some(at)).await?; } }",
        "async fn extend(uow: Unit<'_>) { let mut commissions = uow.commissions(); commissions.set_deadline(&id, None).await?; }",
        "async fn copy(uow: Unit<'_>) { uow.commissions().create(&commission).await?; }",
        "async fn copy(uow: Unit<'_>) { let mut repo = uow.commissions(); repo.create(&commission).await?; }",
        "async fn purge(uow: Unit<'_>) { UnitOfWork::commissions(uow).delete(&id).await?; }",
        "async fn purge(uow: Unit<'_>) { let repo = uow.commissions(); (*repo).delete(&id).await?; }",
        "fn setter() { let set = CommissionRepo::set_maturity; }",
        "async fn extend(uow: Unit<'_>) { tokio::join!(async { uow.commissions().declare_slots(&slots).await }); }",
    ];
    for source in new_sites {
        let file = syn::parse_file(source).expect("the planted use case parses");

        let writes = field_writes("commission/extend.rs", &file);

        assert_eq!(writes.len(), 1, "{source}: {writes:#?}");
    }
    let not_field_writes = [
        "async fn upload(ports: &Ports, key: FileKey) { ports.files.delete(key).await?; }",
        "async fn open(uow: Unit<'_>) { uow.accounts().create(&account, &owner).await?; }",
        "fn make() -> Commission { Commission::create(title, owner, now, None) }",
        "trait Repo { async fn set_maturity(&mut self, id: &CommissionId, maturity: Maturity); }",
    ];
    for source in not_field_writes {
        let file = syn::parse_file(source).expect("the planted source parses");

        let writes = field_writes("commission/other.rs", &file);

        assert_eq!(writes, Vec::<(String, String, String)>::new(), "{source}");
    }
}

// The interim guard: every method of the commission write view takes a capability or is named.

/// The commission write view's methods that take no write capability and that clippy bans in
/// `application`, exactly the ones its settings ban: the hard delete, the view grants and the
/// seat writes. Clippy's ban holds each call; each leaves when its method takes a capability.
const BANNED_WRITES: [&str; 6] = [
    "delete",
    "grant_view",
    "revoke_view",
    "declare_seat",
    "create_seat_invitation",
    "revoke_seat_invitation",
];

/// The commission write view's other methods that take no write capability yet: elements,
/// files and markups. Each leaves when its method takes one.
const OTHER_UNCAPPED: [&str; 4] = ["add_element", "remove_element", "add_file", "add_markup"];

/// The reads on the commission write view: they take no write capability, and never will.
const WRITE_VIEW_READS: [&str; 2] = ["commission_has_facts", "lapsed_deadlines"];

/// The use-case layer's clippy settings.
fn clippy_settings() -> PathBuf {
    crates().join("application/clippy.toml")
}

/// Every quoted string in `settings`, a TOML text, in either quote, past its comments.
fn quoted_strings(settings: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = settings.chars();
    while let Some(character) = chars.next() {
        match character {
            '#' => {
                chars.by_ref().find(|next| *next == '\n');
            }
            '"' | '\'' => {
                let mut text = String::new();
                while let Some(next) = chars.next() {
                    if next == character {
                        break;
                    }
                    if next == '\\' && character == '"' {
                        chars.next();
                        continue;
                    }
                    text.push(next);
                }
                out.push(text);
            }
            _ => {}
        }
    }
    out
}

/// Whether `text` is a path and nothing else: identifiers joined by `::`.
fn is_path(text: &str) -> bool {
    text.split("::").all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
    })
}

/// What breaks the tie between `settings`, a clippy.toml's text, and `listed`: a listed name
/// it doesn't ban, a write-view method it bans that isn't listed, and a ban by any path but
/// `domain::ports::CommissionWrites::<name>`.
fn clippy_tie_findings(settings: &str, listed: &[&str]) -> Vec<String> {
    let banned: Vec<(String, String)> = quoted_strings(settings)
        .into_iter()
        .filter(|text| is_path(text))
        .filter_map(|path| {
            let mut segments = path.rsplit("::");
            let name = segments.next()?.to_owned();
            let on_the_view = segments.next() == Some(COMMISSION_WRITES);
            on_the_view.then_some((path, name))
        })
        .collect();
    let banned_names: BTreeSet<&str> = banned.iter().map(|(_, name)| name.as_str()).collect();
    let listed_names: BTreeSet<&str> = listed.iter().copied().collect();
    let unbanned = listed_names.difference(&banned_names).map(|name| {
        format!("{COMMISSION_WRITES}::{name} is on the banned list, but clippy.toml doesn't ban it")
    });
    let unlisted = banned_names.difference(&listed_names).map(|name| {
        format!("clippy.toml bans {COMMISSION_WRITES}::{name}, which isn't on the banned list")
    });
    let misplaced = banned.iter().filter_map(|(path, name)| {
        let home = format!("domain::ports::{COMMISSION_WRITES}::{name}");
        (*path != home).then(|| format!("clippy.toml bans `{path}`, not `{home}`"))
    });
    unbanned.chain(unlisted).chain(misplaced).collect()
}

#[test]
fn clippy_bans_exactly_the_banned_writes() {
    let settings =
        std::fs::read_to_string(clippy_settings()).expect("application's clippy.toml is readable");

    let findings = clippy_tie_findings(&settings, &BANNED_WRITES);

    assert_eq!(findings, Vec::<String>::new());
    let grant_line = settings
        .lines()
        .find(|line| line.contains("CommissionWrites::grant_view\""))
        .expect("clippy.toml bans grant_view on its own line");
    let unbanned_grant =
        "CommissionWrites::grant_view is on the banned list, but clippy.toml doesn't ban it";
    let unlisted_file =
        "clippy.toml bans CommissionWrites::add_file, which isn't on the banned list";
    let file_ban = "{ path = \"domain::ports::CommissionWrites::add_file\", reason = \"later\" },";
    let closing = settings.rfind(']').expect("clippy.toml's list closes");
    let with_ban = |ban: &str| format!("{}{ban}\n{}", &settings[..closing], &settings[closing..]);
    let elsewhere = grant_line.replacen("domain::ports", "workflow::ports", 1);
    let misplaced_grant = "clippy.toml bans `workflow::ports::CommissionWrites::grant_view`, not `domain::ports::CommissionWrites::grant_view`";
    let commented_out = settings.replacen(grant_line, &format!("# {grant_line}"), 1);
    let planted = [
        (settings.replacen(grant_line, "", 1), unbanned_grant),
        (commented_out, unbanned_grant),
        (with_ban(file_ban), unlisted_file),
        (
            with_ban("'domain::ports::CommissionWrites::add_file',"),
            unlisted_file,
        ),
        (
            settings.replacen(grant_line, &elsewhere, 1),
            misplaced_grant,
        ),
    ];
    for (planted_settings, expected) in planted {
        let planted_findings = clippy_tie_findings(&planted_settings, &BANNED_WRITES);

        assert_eq!(planted_findings, [expected], "{planted_settings}");
    }
}

/// The write view of a commission unit.
const COMMISSION_WRITES: &str = "CommissionWrites";

/// The read view of a commission unit.
const COMMISSION_READS: &str = "CommissionReads";

/// What a unit's `commissions()` vends: the read view and the write view, nothing else.
const COMMISSION_REPO: &str = "CommissionRepo";

/// The read view's methods: lookups and row locks, never a write.
const UNIT_READS: [&str; 5] = [
    "find",
    "find_for_update",
    "standing_for_update",
    "is_participant",
    "tab_for_update",
];

/// The port file holding the commission unit's views.
fn commission_port_file() -> PathBuf {
    crates().join("workflow/src/ports/commission.rs")
}

/// Every name the four lists of uncapped methods hold.
fn listed_uncapped() -> BTreeSet<String> {
    FIELD_WRITES
        .into_iter()
        .chain(BANNED_WRITES)
        .chain(OTHER_UNCAPPED)
        .chain(WRITE_VIEW_READS)
        .map(str::to_owned)
        .collect()
}

/// Every trait named `name` in `file`, inline modules at any depth included.
fn traits_named<'a>(file: &'a syn::File, name: &str) -> Vec<&'a syn::ItemTrait> {
    every_item(&file.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Trait(tr) if tr.ident == name => Some(tr),
            _ => None,
        })
        .collect()
}

/// The commission's id.
const COMMISSION_ID: &str = "CommissionId";

/// A thing not yet made, the thing a create's capability is over.
const NEW: &str = "New";

/// The role of whoever makes a new thing.
const CREATOR: &str = "Creator";

/// The role of a write only the commission's Owner may make.
const AS_OWNER: &str = "AsOwner";

/// The role of a write any Participant of the commission may make.
const AS_PARTICIPANT: &str = "AsParticipant";

/// Each name a capped method's capability is spelled with, and the module the port file must
/// import it from.
const CAPABILITY_HOMES: [(&str, &str); 6] = [
    (WRITE_CAPABILITY, "kernel::ports"),
    (NEW, "kernel::ports"),
    (CREATOR, "kernel::ports"),
    (COMMISSION_ID, "crate::elements::commission"),
    (AS_OWNER, "crate::elements::commission"),
    (AS_PARTICIPANT, "crate::elements::commission"),
];

/// Each role a capped method's capability may name, with the thing it must be over: a write on
/// a commission is over its id, a create over a new one.
const BOUND_ROLES: [(&str, &str); 3] = [
    (AS_OWNER, "CommissionId"),
    (AS_PARTICIPANT, "CommissionId"),
    (CREATOR, "New<CommissionId>"),
];

/// What a parameter beside the capability may not name, since each could stand for an id: an
/// opaque type, a qualified-self path, or `Self`.
const OPAQUE: [&str; 4] = ["impl", "dyn", "as", "Self"];

/// Each name the `use` items in `file` bind without a rename, at any depth, with the path it
/// comes from: `use a::{b::C}` binds `C` from `a::b`, and `use a::b::{self}` binds `b` from `a`.
fn imports(file: &syn::File) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for item in every_item(&file.items) {
        if let Item::Use(item_use) = item {
            let root = if item_use.leading_colon.is_some() {
                vec![String::new()]
            } else {
                Vec::new()
            };
            imported(&item_use.tree, root, &mut out);
        }
    }
    out
}

fn imported(tree: &UseTree, path: Vec<String>, out: &mut Vec<(String, String)>) {
    match tree {
        UseTree::Path(step) => {
            let mut longer = path;
            longer.push(unraw(&step.ident));
            imported(&step.tree, longer, out);
        }
        UseTree::Name(name) if name.ident == "self" => {
            if let Some((last, from)) = path.split_last() {
                out.push((last.clone(), from.join("::")));
            }
        }
        UseTree::Name(name) => out.push((unraw(&name.ident), path.join("::"))),
        UseTree::Group(group) => {
            for item in &group.items {
                imported(item, path.clone(), out);
            }
        }
        UseTree::Rename(_) | UseTree::Glob(_) => {}
    }
}

/// The capability's names `file` holds to their homes, each bound once, by a `use` of its home,
/// and by nothing else; and a finding for every other binding of one. A name the file never
/// binds is not held, and is no finding.
fn held_names(file: &syn::File) -> (BTreeSet<String>, Vec<String>) {
    let names: BTreeSet<String> = CAPABILITY_HOMES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    let bound = bindings(file.to_token_stream(), &names);
    let imported = imports(file);
    let mut held = BTreeSet::new();
    let mut findings = Vec::new();
    for (name, home) in CAPABILITY_HOMES {
        let binders: Vec<&String> = bound
            .iter()
            .filter(|(_, bound_name)| bound_name == name)
            .map(|(binder, _)| binder)
            .collect();
        let from: Vec<&str> = imported
            .iter()
            .filter(|(imported_name, _)| imported_name == name)
            .map(|(_, path)| path.as_str())
            .collect();
        findings.extend(
            binders
                .iter()
                .map(|binder| format!("the port file binds `{name}` by {binder}")),
        );
        if from.len() > 1 || from.iter().any(|path| *path != home) {
            findings.push(format!(
                "the port file imports `{name}` from {from:?}, not once from `{home}`"
            ));
        }
        if binders.is_empty() && from == [home] {
            held.insert(name.to_owned());
        }
    }
    (held, findings)
}

/// The names that stand for a commission's id in `file`: [`COMMISSION_ID`] and every alias of
/// it the workspace or `file` makes.
fn commission_id_names(file: &syn::File) -> BTreeSet<String> {
    static WORKSPACE: OnceLock<BTreeSet<String>> = OnceLock::new();
    let workspace = WORKSPACE.get_or_init(|| with_aliases(&[COMMISSION_ID], &workspace_sources()));
    let seeds: Vec<&str> = workspace.iter().map(String::as_str).collect();
    aliases_in_file(&seeds, file)
}

/// The name and arguments of `ty` when it is a path of one segment, with no leading `::` and
/// no qualified self.
fn one_segment(ty: &Type) -> Option<(String, &syn::PathArguments)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segments = &path.path.segments;
    let plain = path.qself.is_none() && path.path.leading_colon.is_none() && segments.len() == 1;
    let segment = segments.first().filter(|_| plain)?;
    Some((unraw(&segment.ident), &segment.arguments))
}

/// The one bare name `ty` is: a one-segment path with no arguments.
fn bare_name(ty: &Type) -> Option<String> {
    let (name, arguments) = one_segment(ty)?;
    arguments.is_none().then_some(name)
}

/// `ty` as a capability's thing is written: a bare name, or a bare name over one bare name
/// (`New<CommissionId>`).
fn thing_shape(ty: &Type) -> Option<String> {
    let (name, arguments) = one_segment(ty)?;
    let syn::PathArguments::AngleBracketed(angled) = arguments else {
        return arguments.is_none().then_some(name);
    };
    let inner: Vec<&syn::GenericArgument> = angled.args.iter().collect();
    let [syn::GenericArgument::Type(inner)] = inner.as_slice() else {
        return None;
    };
    let inner = bare_name(inner)?;
    Some(format!("{name}<{inner}>"))
}

/// The thing and the role of `ty` when it is `&WriteCapability<Thing, Role>`: a shared
/// reference to a one-segment path with two type arguments, its role a bare name.
fn capability_arguments(ty: &Type) -> Option<(String, String)> {
    let Type::Reference(reference) = ty else {
        return None;
    };
    let (name, arguments) = one_segment(&reference.elem)?;
    let syn::PathArguments::AngleBracketed(angled) = arguments else {
        return None;
    };
    let shared = reference.mutability.is_none() && name == WRITE_CAPABILITY;
    let types: Vec<&Type> = angled
        .args
        .iter()
        .map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let [thing, role] = types.as_slice() else {
        return None;
    };
    let thing = thing_shape(thing).filter(|_| shared)?;
    Some((thing, bare_name(role)?))
}

/// Whether `role` is a concrete role and `thing` the one it is bound to, with the capability's
/// name and every name they spell held to its home.
fn bound_to(thing: &str, role: &str, held: &BTreeSet<String>) -> bool {
    let paired = BOUND_ROLES
        .iter()
        .any(|(bound_role, bound_thing)| *bound_role == role && *bound_thing == thing);
    let thing_names = thing.split(['<', '>']).filter(|part| !part.is_empty());
    let mut spelled = [WRITE_CAPABILITY, role].into_iter().chain(thing_names);
    paired && spelled.all(|name| held.contains(name))
}

/// Whether `ty`, a parameter beside the capability, can't carry a commission's id: it names
/// none of `ids`, nothing [`OPAQUE`], and no macro.
fn carries_no_id(ty: &Type, ids: &BTreeSet<String>) -> bool {
    let tokens = ty.to_token_stream();
    let named = identifiers(tokens.clone())
        .iter()
        .any(|ident| ids.contains(ident) || OPAQUE.contains(&ident.as_str()));
    let macro_call = levels(tokens)
        .iter()
        .flatten()
        .any(|tree| matches!(tree, TokenTree::Punct(punct) if punct.as_char() == '!'));
    !named && !macro_call
}

/// Whether `signature` is capped: exactly one parameter names [`WRITE_CAPABILITY`], and it is
/// `&WriteCapability<Thing, Role>`, its role concrete and its thing the role's, held to their
/// homes; no other parameter could carry a commission's id (`ids`), so the capability stands in
/// place of the id; and the method has no generics but lifetimes and no where clause.
fn is_capped(signature: &syn::Signature, held: &BTreeSet<String>, ids: &BTreeSet<String>) -> bool {
    let generics = &signature.generics;
    let generic = generics.where_clause.is_some()
        || generics
            .params
            .iter()
            .any(|param| !matches!(param, syn::GenericParam::Lifetime(_)));
    let typed: Vec<&Type> = signature
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => Some(typed.ty.as_ref()),
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let (capabilities, others): (Vec<&Type>, Vec<&Type>) = typed.into_iter().partition(|ty| {
        identifiers(ty.to_token_stream())
            .iter()
            .any(|ident| ident == WRITE_CAPABILITY)
    });
    let [capability] = capabilities.as_slice() else {
        return false;
    };
    let bound =
        capability_arguments(capability).is_some_and(|(thing, role)| bound_to(&thing, &role, held));
    let free = others.iter().all(|ty| carries_no_id(ty, ids));
    !generic && bound && free
}

/// The name a finding gives a trait item: a method's name, an associated item's kind and name,
/// or anything else as written.
fn trait_item_name(item: &TraitItem) -> String {
    match item {
        TraitItem::Fn(method) => unraw(&method.sig.ident),
        TraitItem::Const(constant) => format!("const {}", unraw(&constant.ident)),
        TraitItem::Type(alias) => format!("type {}", unraw(&alias.ident)),
        other => other.to_token_stream().to_string(),
    }
}

/// A trait's items sorted by authority: each capped method (see [`is_capped`]), and everything
/// else, an item that isn't a method included.
#[derive(Debug, Default)]
struct Authority {
    capped: BTreeSet<String>,
    uncapped: BTreeSet<String>,
}

/// `tr`'s items, sorted by authority, with `held` the capability's names its file holds to
/// their homes and `ids` the names that stand for a commission's id there.
fn authority(tr: &syn::ItemTrait, held: &BTreeSet<String>, ids: &BTreeSet<String>) -> Authority {
    let mut sorted = Authority::default();
    for item in &tr.items {
        let capped = matches!(item, TraitItem::Fn(method) if is_capped(&method.sig, held, ids));
        let name = trait_item_name(item);
        if capped {
            sorted.capped.insert(name);
        } else {
            sorted.uncapped.insert(name);
        }
    }
    sorted
}

/// The items of `tr`, a trait in `file`, sorted by authority.
fn file_authority(file: &syn::File, tr: &syn::ItemTrait) -> Authority {
    let (held, _) = held_names(file);
    authority(tr, &held, &commission_id_names(file))
}

/// Each method of `tr` with a default body: a body could reach any write on any id.
fn default_body_findings(tr: &syn::ItemTrait) -> Vec<String> {
    tr.items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Fn(method) if method.default.is_some() => Some(format!(
                "{COMMISSION_WRITES}::{} has a default body",
                unraw(&method.sig.ident)
            )),
            _ => None,
        })
        .collect()
}

/// The port scan's findings in `file`: anything but one [`COMMISSION_WRITES`], each binding of
/// the capability's names other than the one `use` of each's home, each uncapped item `listed`
/// lacks, each name in `listed` that isn't an uncapped item, and each default body.
fn port_findings(file: &syn::File, listed: &BTreeSet<String>) -> Vec<String> {
    let found = traits_named(file, COMMISSION_WRITES);
    let [writes] = found.as_slice() else {
        let count = found.len();
        return vec![format!(
            "{count} `trait {COMMISSION_WRITES}` found, not one"
        )];
    };
    let (held, homes) = held_names(file);
    let sorted = authority(writes, &held, &commission_id_names(file));
    let unlisted = sorted.uncapped.difference(listed).map(|name| {
        format!("{COMMISSION_WRITES}::{name} takes no write capability and is on no list")
    });
    let stale = listed.difference(&sorted.uncapped).map(|name| {
        if sorted.capped.contains(name) {
            format!("{COMMISSION_WRITES}::{name} takes a write capability: it leaves its list")
        } else {
            format!("{COMMISSION_WRITES}::{name} is listed but gone from the trait")
        }
    });
    let defaults = default_body_findings(writes);
    homes
        .into_iter()
        .chain(unlisted)
        .chain(stale)
        .chain(defaults)
        .collect()
}

/// Each bound in `tr`'s supertraits, as written.
fn supertraits(tr: &syn::ItemTrait) -> Vec<String> {
    tr.supertraits
        .iter()
        .map(|bound| bound.to_token_stream().to_string())
        .collect()
}

/// What breaks trait `name`'s shape in `tr`: generics or a where clause, or supertraits other
/// than `bounds`.
fn trait_shape_findings(tr: &syn::ItemTrait, name: &str, bounds: &[&str]) -> Vec<String> {
    let mut findings = Vec::new();
    if !tr.generics.params.is_empty() || tr.generics.where_clause.is_some() {
        let generics = tr.generics.to_token_stream();
        let where_clause = tr.generics.where_clause.to_token_stream();
        findings.push(format!(
            "trait {name}: generics `{generics} {where_clause}`"
        ));
    }
    let held = supertraits(tr);
    if held != bounds {
        findings.push(format!(
            "trait {name}: supertraits {held:?}, not {bounds:?}"
        ));
    }
    findings
}

/// What breaks one trait's pin: not exactly one trait `name` in `file`, generics or a where
/// clause, supertraits other than `bounds`, or items other than `items`, each named.
fn pinned_trait_findings(
    file: &syn::File,
    name: &str,
    bounds: &[&str],
    items: &[&str],
) -> Vec<String> {
    let found = traits_named(file, name);
    let [tr] = found.as_slice() else {
        let count = found.len();
        return vec![format!("{count} `trait {name}` found, not one")];
    };
    let mut findings = trait_shape_findings(tr, name, bounds);
    let pinned: BTreeSet<String> = items.iter().map(|item| (*item).to_owned()).collect();
    let all: BTreeSet<String> = tr.items.iter().map(trait_item_name).collect();
    findings.extend(
        all.difference(&pinned)
            .map(|item| format!("{name}::{item} is not one of its pinned items")),
    );
    findings.extend(
        pinned
            .difference(&all)
            .map(|item| format!("{name}::{item} is pinned but gone from the trait")),
    );
    findings
}

/// The side doors around the write view in `file`: the repo a unit vends is exactly the read
/// view and the write view with nothing of its own, the read view holds only its reads, and
/// neither view gains a supertrait or a where clause.
fn side_door_findings(file: &syn::File) -> Vec<String> {
    let repo = pinned_trait_findings(
        file,
        COMMISSION_REPO,
        &[COMMISSION_READS, COMMISSION_WRITES],
        &[],
    );
    let reads = pinned_trait_findings(file, COMMISSION_READS, &["Send"], &UNIT_READS);
    let write_shape: Vec<String> = traits_named(file, COMMISSION_WRITES)
        .into_iter()
        .flat_map(|tr| trait_shape_findings(tr, COMMISSION_WRITES, &["Send"]))
        .collect();
    repo.into_iter().chain(reads).chain(write_shape).collect()
}

/// The imports a planted port trait's capability is spelled with, each from its home.
const PORT_PRELUDE: &str = "use crate::elements::commission::{AsOwner, AsParticipant, CommissionId};\nuse kernel::ports::{Creator, New, WriteCapability};\n";

/// A security review's plant on the write view: a look-alike capability by a longer path, and
/// a create's capability beside a free id, each with a default body.
const HERON_S_PORT_PLANT: &str = r"
    /// Moves the commission's lifecycle step.
    async fn set_lifecycle(
        &mut self,
        capability: &crate::elements::commission::fact::WriteCapability<
            CommissionId,
            crate::elements::commission::AsOwner,
        >,
        step: crate::elements::commission::LifecycleStep,
    ) -> anyhow::Result<bool> {
        let _ = (capability.0, step);
        Ok(false)
    }

    /// Sets any commission's maturity under a creator's capability.
    async fn set_maturity_under(
        &mut self,
        capability: &WriteCapability<New<CommissionId>, Creator>,
        id: &CommissionId,
        maturity: Maturity,
    ) -> anyhow::Result<()> {
        let _ = capability;
        self.set_maturity(id, maturity).await
    }
";

#[test]
fn every_commission_write_takes_a_capability_or_is_named() {
    let file = parse(&commission_port_file());
    let listed = listed_uncapped();

    let findings = port_findings(&file, &listed);
    let side_doors = side_door_findings(&file);

    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(side_doors, Vec::<String>::new());
    let writes = traits_named(&file, COMMISSION_WRITES);
    assert_eq!(writes.len(), 1, "the scan finds trait {COMMISSION_WRITES}");
    let sorted = file_authority(&file, writes[0]);
    assert_eq!(
        sorted.capped,
        BTreeSet::from(["create".to_owned()]),
        "only create takes the gate's create capability, in place of the id"
    );
    let (held, homes) = held_names(&file);
    assert_eq!(homes, Vec::<String>::new());
    for name in [WRITE_CAPABILITY, NEW, CREATOR, COMMISSION_ID] {
        assert!(
            held.contains(name),
            "the port file imports {name} from its home"
        );
    }
    assert_eq!(default_body_findings(writes[0]), Vec::<String>::new());
    let list_lengths =
        FIELD_WRITES.len() + BANNED_WRITES.len() + OTHER_UNCAPPED.len() + WRITE_VIEW_READS.len();
    assert_eq!(listed.len(), list_lengths, "no name sits on two lists");
}

#[test]
fn the_port_scan_names_each_planted_write() {
    let source =
        std::fs::read_to_string(commission_port_file()).expect("the port file is readable");
    let listed = listed_uncapped();
    let writes_header = "pub trait CommissionWrites: Send {";
    let reads_header = "pub trait CommissionReads: Send {";
    let repo_header = "pub trait CommissionRepo: CommissionReads + CommissionWrites {}";
    for header in [writes_header, reads_header, repo_header] {
        assert!(source.contains(header), "the port file holds `{header}`");
    }
    let set_lifecycle = "\n    async fn set_lifecycle(&mut self, id: &CommissionId, step: LifecycleStep) -> anyhow::Result<bool>;\n";
    let lifecycle_planted =
        source.replacen(writes_header, &format!("{writes_header}{set_lifecycle}"), 1);
    let lifecycle_file = syn::parse_file(&lifecycle_planted).expect("the planted port parses");

    let lifecycle_findings = port_findings(&lifecycle_file, &listed);

    assert_eq!(
        lifecycle_findings,
        ["CommissionWrites::set_lifecycle takes no write capability and is on no list"]
    );
    let none = BTreeSet::new();
    let unlisted_lifecycle =
        "CommissionWrites::set_lifecycle takes no write capability and is on no list";
    let lifecycle_by_owner = "\n    async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, step: LifecycleStep) -> anyhow::Result<bool>;\n";
    let errors_line = "pub use super::errors";
    assert!(
        source.contains(errors_line),
        "the port file holds `{errors_line}`"
    );
    let owner_import = format!("use crate::elements::commission::AsOwner;\n\n{errors_line}");
    let owner_planted = source.replacen(errors_line, &owner_import, 1).replacen(
        writes_header,
        &format!("{writes_header}{lifecycle_by_owner}"),
        1,
    );
    let capped = [
        (owner_planted, &listed),
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, step: LifecycleStep) -> anyhow::Result<bool>; }}"
            ),
            &none,
        ),
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle<'a>(&mut self, note: &str, capability: &'a WriteCapability<CommissionId, AsParticipant>); }}"
            ),
            &none,
        ),
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle(&mut self, capability: &WriteCapability<New<CommissionId>, Creator>, birth: &CommissionBirth, commission: &NewCommission) -> anyhow::Result<()>; }}"
            ),
            &none,
        ),
    ];
    for (planted, listed) in capped {
        let file = syn::parse_file(&planted).expect("the planted trait parses");

        let findings = port_findings(&file, listed);

        assert_eq!(findings, Vec::<String>::new(), "{planted}");
        let sorted = file_authority(&file, traits_named(&file, COMMISSION_WRITES)[0]);
        assert!(
            sorted.capped.contains("set_lifecycle"),
            "{planted}: {sorted:?}"
        );
    }
    let uncapped = [
        "async fn set_lifecycle(&mut self, capability: WriteCapability<CommissionId, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &Cap);",
        "async fn set_lifecycle<C>(&mut self, capability: &C);",
        "async fn set_lifecycle(&mut self, capability: &impl AsRef<WriteCapability<CommissionId, AsOwner>>);",
        "async fn set_lifecycle(&mut self, capability: &<Gate as Issue>::WriteCapability);",
        "async fn set_lifecycle(&mut self, step: LifecycleStep, capability: &kernel::ports::WriteCapability<CommissionId, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &crate::elements::commission::fact::WriteCapability<CommissionId, crate::elements::commission::AsOwner>, step: LifecycleStep);",
        "async fn set_lifecycle(&mut self, capability: &::kernel::ports::WriteCapability<CommissionId, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &mut WriteCapability<CommissionId, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, R>);",
        "async fn set_lifecycle<R>(&mut self, capability: &WriteCapability<CommissionId, R>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, Creator>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<New<CommissionId>, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<New<CommissionId>, AsParticipant>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<New<New<CommissionId>>, Creator>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, crate::elements::commission::AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner, AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<(CommissionId), AsOwner>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, again: &WriteCapability<CommissionId, AsParticipant>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, again: Option<WriteCapability<CommissionId, AsOwner>>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<New<CommissionId>, Creator>, id: &CommissionId, maturity: Maturity);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: Option<&CommissionId>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: CommissionId);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, ids: &[CommissionId]);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, pair: (u8, CommissionId));",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: &Id);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: impl Into<Uuid>);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: &dyn Any);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: &<AsOwner as RoleTable>::Thing);",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, id: id_type!());",
        "async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>) where Self: Sized;",
    ];
    for method in uncapped {
        let planted = format!(
            "{PORT_PRELUDE}type Cap = WriteCapability<CommissionId, AsOwner>;\ntype Id = CommissionId;\ntrait CommissionWrites: Send {{ {method} }}"
        );
        let file = syn::parse_file(&planted).expect("the planted trait parses");

        let findings = port_findings(&file, &none);

        assert_eq!(findings, [unlisted_lifecycle], "{method}");
    }
    let by_owner = "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>); }";
    let homes = [
        (
            format!(
                "{PORT_PRELUDE}pub struct WriteCapability<T, R>(pub T, pub PhantomData<R>);\n{by_owner}"
            ),
            vec!["the port file binds `WriteCapability` by `struct`"],
        ),
        (
            format!("{PORT_PRELUDE}type WriteCapability<T, R> = (T, R);\n{by_owner}"),
            vec!["the port file binds `WriteCapability` by `type`"],
        ),
        (
            format!(
                "use crate::elements::commission::{{AsOwner, CommissionId}};\nuse kernel::ports::Creator as WriteCapability;\n{by_owner}"
            ),
            vec!["the port file binds `WriteCapability` by `as`"],
        ),
        (
            format!(
                "use crate::elements::commission::{{AsOwner, CommissionId}};\nuse crate::elements::commission::fact::WriteCapability;\n{by_owner}"
            ),
            vec![
                "the port file imports `WriteCapability` from [\"crate::elements::commission::fact\"], not once from `kernel::ports`",
            ],
        ),
        (
            format!(
                "{PORT_PRELUDE}mod inner {{ use kernel::ports::WriteCapability; }}\n{by_owner}"
            ),
            vec![
                "the port file imports `WriteCapability` from [\"kernel::ports\", \"kernel::ports\"], not once from `kernel::ports`",
            ],
        ),
        (
            format!(
                "use crate::elements::commission::{{fact::AsOwner, CommissionId}};\nuse kernel::ports::WriteCapability;\n{by_owner}"
            ),
            vec![
                "the port file imports `AsOwner` from [\"crate::elements::commission::fact\"], not once from `crate::elements::commission`",
            ],
        ),
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle<AsOwner>(&mut self, capability: &WriteCapability<CommissionId, AsOwner>); }}"
            ),
            vec!["the port file binds `AsOwner` by a generic parameter list"],
        ),
    ];
    for (planted, home_findings) in homes {
        let file = syn::parse_file(&planted).expect("the planted port parses");

        let findings = port_findings(&file, &none);

        let mut expected: Vec<&str> = home_findings;
        expected.push(unlisted_lifecycle);
        assert_eq!(findings, expected, "{planted}");
    }
    let lifecycle_default = "CommissionWrites::set_lifecycle has a default body";
    let default_bodies = [
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, step: LifecycleStep) -> anyhow::Result<bool> {{ let _ = (capability, step); Ok(false) }} }}"
            ),
            vec![lifecycle_default],
        ),
        (
            format!(
                "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_lifecycle(&mut self, id: &CommissionId) {{ let _ = WriteCapability::thing; }} }}"
            ),
            vec![unlisted_lifecycle, lifecycle_default],
        ),
    ];
    for (planted, expected) in default_bodies {
        let file = syn::parse_file(&planted).expect("the planted trait parses");

        let findings = port_findings(&file, &none);

        assert_eq!(findings, expected, "{planted}");
    }
    let last_write = "    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>>;\n";
    assert!(
        source.contains(last_write),
        "the port file holds `{last_write}`"
    );
    let heron_s_plant = format!("{last_write}{HERON_S_PORT_PLANT}");
    let heron_planted = source.replacen(last_write, &heron_s_plant, 1);
    let heron_file = syn::parse_file(&heron_planted).expect("the planted port parses");

    let heron_findings = port_findings(&heron_file, &listed);

    assert_eq!(
        heron_findings,
        [
            "CommissionWrites::set_lifecycle takes no write capability and is on no list",
            "CommissionWrites::set_maturity_under takes no write capability and is on no list",
            "CommissionWrites::set_lifecycle has a default body",
            "CommissionWrites::set_maturity_under has a default body",
        ]
    );
    let set_archived: BTreeSet<String> = BTreeSet::from(["set_archived".to_owned()]);
    let leaves_its_list = format!(
        "{PORT_PRELUDE}trait CommissionWrites: Send {{ async fn set_archived(&mut self, capability: &WriteCapability<CommissionId, AsOwner>); }}"
    );
    let named_findings = [
        (
            "trait CommissionWrites: Send { writes!(); }",
            &none,
            vec!["CommissionWrites::writes ! () ; takes no write capability and is on no list"],
        ),
        (
            "trait CommissionWrites: Send { const LIFECYCLE: bool; }",
            &none,
            vec!["CommissionWrites::const LIFECYCLE takes no write capability and is on no list"],
        ),
        (
            "trait CommissionWrites: Send {}",
            &set_archived,
            vec!["CommissionWrites::set_archived is listed but gone from the trait"],
        ),
        (
            leaves_its_list.as_str(),
            &set_archived,
            vec!["CommissionWrites::set_archived takes a write capability: it leaves its list"],
        ),
        (
            "trait CommissionReads: Send {}",
            &none,
            vec!["0 `trait CommissionWrites` found, not one"],
        ),
        (
            "trait CommissionWrites: Send {}\nmod hidden { trait CommissionWrites: Send {} }",
            &none,
            vec!["2 `trait CommissionWrites` found, not one"],
        ),
    ];
    for (planted, listed, expected) in named_findings {
        let file = syn::parse_file(planted).expect("the planted trait parses");

        let findings = port_findings(&file, listed);

        assert_eq!(findings, expected, "{planted}");
    }
    let side_doors = [
        (
            repo_header,
            "pub trait CommissionRepo: CommissionReads + CommissionWrites + CommissionFields {}",
            "trait CommissionRepo: supertraits",
        ),
        (
            repo_header,
            "pub trait CommissionRepo: CommissionReads + CommissionWrites { async fn set_lifecycle(&mut self); }",
            "CommissionRepo::set_lifecycle is not one of its pinned items",
        ),
        (
            repo_header,
            "pub trait CommissionRepo: CommissionReads + CommissionWrites where Self: CommissionFields {}",
            "trait CommissionRepo: generics",
        ),
        (
            reads_header,
            "pub trait CommissionReads: Send {\n    async fn set_lifecycle(&mut self, id: &CommissionId) -> anyhow::Result<bool>;\n",
            "CommissionReads::set_lifecycle is not one of its pinned items",
        ),
        (
            reads_header,
            "pub trait CommissionReads: Send + CommissionFields {",
            "trait CommissionReads: supertraits",
        ),
        (
            writes_header,
            "pub trait CommissionWrites: Send + CommissionFields {",
            "trait CommissionWrites: supertraits",
        ),
    ];
    for (header, replacement, expected) in side_doors {
        let planted = source.replacen(header, replacement, 1);
        let file = syn::parse_file(&planted).expect("the planted port parses");

        let findings = side_door_findings(&file);

        assert_eq!(findings.len(), 1, "{replacement}: {findings:?}");
        assert!(
            findings[0].starts_with(expected),
            "{replacement}: {findings:?}"
        );
    }
}

// Commission writes are pinned at the store: whichever port a write is called through, the
// commission tables change only inside the commission write view.

/// The commission's table, its query folder and its generated query module.
const COMMISSION_TABLE: &str = "commission";

/// The prefix every other commission table's name starts with.
const COMMISSION_TABLE_PREFIX: &str = "commission_";

/// The changelog port's own table, written under `queries/changelog/`.
const CHANGELOG_TABLE: &str = "commission_changelog";

/// The SQL words a written table follows, past any of [`SQL_WRITE_FILLERS`].
const SQL_WRITES: [&str; 6] = ["INSERT", "UPDATE", "DELETE", "MERGE", "TRUNCATE", "COPY"];

/// The SQL words that may stand between a write and its table.
const SQL_WRITE_FILLERS: [&str; 4] = ["INTO", "FROM", "ONLY", "TABLE"];

/// The PostgreSQL adapter's generated query module, and its list of write functions.
const QUERIES: &str = "queries";

/// The generated list of every write function, as `namespace::function`.
const WRITE_QUERY_FNS: &str = "WRITE_QUERY_FNS";

/// The generated file, which defines the query functions and calls none.
const GENERATED_QUERIES: &str = "queries.rs";

/// Each adapter's commission file, relative to its `src`.
const COMMISSION_FILE: &str = "commission.rs";

/// The PostgreSQL adapter's unit file, relative to its `src`.
const PG_UNIT_FILE: &str = "uow.rs";

/// The PostgreSQL commission write view.
const PG_WRITE_VIEW: &str = "PgCommissionWrites";

/// The one function that builds [`PG_WRITE_VIEW`]: the unit's `commissions()`.
const PG_UNIT_COMMISSIONS: &str = "PgUnitOfWork::commissions";

/// The in-memory adapter's crate root, relative to its `src`.
const MEM_LIB_FILE: &str = "lib.rs";

/// The in-memory commission write view.
const MEM_WRITE_VIEW: &str = "MemCommissionWrites";

/// The one function that builds [`MEM_WRITE_VIEW`]: the unit's `commissions()`.
const MEM_UNIT_COMMISSIONS: &str = "MemUnitOfWork::commissions";

/// The in-memory backend, and its field holding the commission map.
const MEM_BACKEND: &str = "MemBackend";

/// The in-memory backend's commission map, a field also named by each column's card list.
const MEM_MAP: &str = "commissions";

/// The in-memory commission row.
const STORED_COMMISSION: &str = "StoredCommission";

/// The one insert path, the only place a [`STORED_COMMISSION`] is built.
const INSERT_COMMISSION: &str = "insert_commission";

/// The pieces besides the write view's methods that may change the in-memory commission map,
/// by file and context: the insert path, the lifecycle seeder, and the unit's staging and
/// merge.
const MEM_MAP_WRITERS: [(&str, &str); 4] = [
    (COMMISSION_FILE, INSERT_COMMISSION),
    (COMMISSION_FILE, "MemBackend::seed_lifecycle_step"),
    (MEM_LIB_FILE, "MemBackend::stage"),
    (MEM_LIB_FILE, "MemBackend::merge"),
];

/// The one piece that builds a [`MEM_BACKEND`] naming its commission map.
const MEM_STAGE: (&str, &str) = (MEM_LIB_FILE, "MemBackend::stage");

/// A column's card list is a field also named `commissions`: the places that hand it on or set
/// it, by file and context, each with how often.
const COLUMN_CARD_LISTS: [(&str, &str, usize); 3] = [
    ("workflow.rs", "StoredColumn::rebuild", 1),
    ("workflow.rs", "MemWorkflowWrites::set_indexes", 1),
    ("workflow.rs", "MemColumnWrites::set_commissions", 1),
];

/// The mutex methods the commission map is reached past.
const MUTEX_LOCKS: [&str; 2] = ["lock", "try_lock"];

/// The methods that pass the map, or its lock, on unchanged; `clone` too before the lock.
const MAP_PASSES: [&str; 8] = [
    "expect",
    "unwrap",
    "unwrap_or_else",
    "map_err",
    "as_ref",
    "as_deref",
    "borrow",
    "deref",
];

/// The methods that only read the map, or a column's card list.
const MAP_READS: [&str; 10] = [
    "get",
    "get_key_value",
    "contains_key",
    "contains",
    "iter",
    "keys",
    "values",
    "len",
    "is_empty",
    "clone",
];

/// The PostgreSQL adapter's folder.
fn adapter_pg() -> PathBuf {
    crates().join("adapter-pg")
}

/// The in-memory adapter's sources.
fn adapter_mem_src() -> PathBuf {
    crates().join("adapter-mem/src")
}

/// A crate's production sources under `src`, each with its path relative to `src`, the files
/// in `skipped` aside.
fn sources_under(src: &Path, skipped: &[&str]) -> Vec<(String, String)> {
    walk(src)
        .production
        .iter()
        .map(|path| (relative(path, src), path))
        .filter(|(file, _)| !skipped.contains(&file.as_str()))
        .map(|(file, path)| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            (file, source)
        })
        .collect()
}

/// `files` with the file `file` holding `source` instead, or added.
fn with_file(files: &[(String, String)], file: &str, source: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = files
        .iter()
        .filter(|(name, _)| name != file)
        .cloned()
        .collect();
    out.push((file.to_owned(), source.to_owned()));
    out
}

/// `files` with `addition` put after `anchor` in the file `file`, which holds it.
fn planted_after(
    files: &[(String, String)],
    file: &str,
    anchor: &str,
    addition: &str,
) -> Vec<(String, String)> {
    let (_, source) = files
        .iter()
        .find(|(name, _)| name == file)
        .unwrap_or_else(|| panic!("the tree holds {file}"));
    assert!(source.contains(anchor), "{file} holds `{anchor}`");
    let planted = source.replacen(anchor, &format!("{anchor}\n{addition}\n"), 1);
    with_file(files, file, &planted)
}

/// The text of a table name in SQL: a bare name folded to lower case as PostgreSQL folds it,
/// or a quoted one as written.
fn table_name(word: Option<&SqlWord>) -> Option<String> {
    match word {
        Some(SqlWord::Bare(bare)) => Some(bare.to_ascii_lowercase()),
        Some(SqlWord::Quoted(quoted)) => Some(quoted.clone()),
        _ => None,
    }
}

/// Whether `name` is a commission table other than the changelog's.
fn is_a_commission_table(name: &str) -> bool {
    let owned = name == COMMISSION_TABLE || name.starts_with(COMMISSION_TABLE_PREFIX);
    owned && name != CHANGELOG_TABLE
}

/// The index of the name a table reference starting at `at` ends on: `t`, or the last part of
/// `schema.t`.
fn table_name_at(words: &[SqlWord], at: usize) -> usize {
    let mut name = at;
    while words.get(name + 1) == Some(&SqlWord::Other('.'))
        && matches!(
            words.get(name + 2),
            Some(SqlWord::Bare(_) | SqlWord::Quoted(_))
        )
    {
        name += 2;
    }
    name
}

/// Each commission table `sql` writes, its comments stripped: the table after each of
/// [`SQL_WRITES`] past any of [`SQL_WRITE_FILLERS`], and each table in a list after it.
fn commission_tables_written(sql: &str) -> Vec<String> {
    let words = sql_words(&without_sql_comments(sql));
    let mut out = Vec::new();
    for at in 0..words.len() {
        let writes = SQL_WRITES
            .iter()
            .any(|keyword| is_bare(words.get(at), keyword));
        if !writes {
            continue;
        }
        let mut table = at + 1;
        loop {
            while SQL_WRITE_FILLERS
                .iter()
                .any(|filler| is_bare(words.get(table), filler))
            {
                table += 1;
            }
            let name_at = table_name_at(&words, table);
            if let Some(name) = table_name(words.get(name_at))
                && is_a_commission_table(&name)
            {
                out.push(name);
            }
            let mut past = name_at + 1;
            if words.get(past) == Some(&SqlWord::Other('*')) {
                past += 1;
            }
            if words.get(past) != Some(&SqlWord::Other(',')) {
                break;
            }
            table = past + 1;
        }
    }
    out
}

/// What `sql` holds that only the commission's query folder may: a write to a commission
/// table, or a Unicode-escaped identifier (which can spell any table's name).
fn commission_sql_findings(sql: &str) -> Vec<String> {
    let mut out: Vec<String> = commission_tables_written(sql)
        .into_iter()
        .map(|table| format!("writes table `{table}`"))
        .collect();
    if holds_a_unicode_identifier(sql) {
        out.push("holds a Unicode-escaped identifier".to_owned());
    }
    out
}

/// Each statement file under `folder`, with its path relative to `queries` and its text.
fn sql_files(folder: &Path, queries: &Path, out: &mut Vec<(String, String)>) {
    let entries = std::fs::read_dir(folder).expect("a readable query folder");
    for entry in entries {
        let path = entry.expect("a query folder entry").path();
        if path.is_dir() {
            sql_files(&path, queries, out);
        } else if path.extension().is_some_and(|extension| extension == "sql") {
            let sql = std::fs::read_to_string(&path).expect("a readable statement");
            out.push((relative(&path, queries), sql));
        }
    }
}

/// Every statement under the PostgreSQL adapter's `queries/`, by path relative to it.
fn adapter_queries() -> Vec<(String, String)> {
    let queries = adapter_pg().join(QUERIES);
    let mut out = Vec::new();
    sql_files(&queries, &queries, &mut out);
    out.sort();
    out
}

/// Each statement in `files` (by path relative to `queries/`) outside the commission's folder
/// that writes a commission table or holds a Unicode-escaped identifier.
fn misplaced_commission_sql(files: &[(String, String)]) -> Vec<String> {
    let commission_folder = format!("{COMMISSION_TABLE}/");
    files
        .iter()
        .filter(|(file, _)| !file.starts_with(&commission_folder))
        .flat_map(|(file, sql)| {
            commission_sql_findings(sql)
                .into_iter()
                .map(move |finding| format!("adapter-pg/queries/{file}: {finding}"))
        })
        .collect()
}

/// The generated commission write functions: each `commission::…` entry of the generated
/// [`WRITE_QUERY_FNS`] list, by function name.
fn commission_write_fns(generated: &syn::File) -> BTreeSet<String> {
    let prefix = format!("{COMMISSION_TABLE}::");
    every_item(&generated.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Static(list) if list.ident == WRITE_QUERY_FNS => Some(list),
            _ => None,
        })
        .flat_map(|list| string_literals(list.expr.to_token_stream()))
        .filter_map(|entry| entry.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

/// `seeds`, and every name `sources` bind to one of them at any depth, function bodies
/// included: a rename (`… as x`) or a type alias naming one, through any chain.
fn token_aliases(seeds: &[&str], sources: &[String]) -> BTreeSet<String> {
    let all_levels: Vec<Vec<TokenTree>> = sources
        .iter()
        .flat_map(|source| {
            let file = syn::parse_file(source).expect("the source parses");
            levels(file.to_token_stream())
        })
        .collect();
    let mut names: BTreeSet<String> = seeds.iter().map(|seed| (*seed).to_owned()).collect();
    loop {
        let mut grown = names.clone();
        for level in &all_levels {
            for at in 0..level.len() {
                let Some(ident) = ident_at(level, at) else {
                    continue;
                };
                if names.contains(&ident)
                    && ident_at(level, at + 1).as_deref() == Some("as")
                    && let Some(bound) = ident_at(level, at + 2)
                    && bound != "_"
                {
                    grown.insert(bound);
                }
                if ident == "type"
                    && let Some(alias) = ident_at(level, at + 1)
                {
                    let end = (at..level.len())
                        .find(|index| punct_at(level, *index, ';'))
                        .unwrap_or(level.len());
                    let names_one = level[(at + 2).min(end)..end].iter().any(|tree| {
                        identifiers(TokenStream::from(tree.clone()))
                            .iter()
                            .any(|name| names.contains(name))
                    });
                    if names_one {
                        grown.insert(alias);
                    }
                }
            }
        }
        if grown == names {
            return names;
        }
        names = grown;
    }
}

/// What one path element after `queries::` reaches: the commission module, a glob, or an
/// alias of `queries` itself.
fn query_element_reach(element: &[TokenTree]) -> Option<String> {
    let first = ident_at(element, 0);
    if first.as_deref() == Some(COMMISSION_TABLE) {
        return Some(format!("`{QUERIES}::{COMMISSION_TABLE}`"));
    }
    if punct_at(element, 0, '*') {
        return Some(format!("a glob from `{QUERIES}`"));
    }
    let renamed = first.as_deref() == Some("self") && ident_at(element, 1).as_deref() == Some("as");
    renamed.then(|| format!("an alias of `{QUERIES}`"))
}

/// Each way `level` reaches the generated commission module from outside the commission file:
/// a `queries::commission` path, `commission` in a group under `queries::`, a glob from
/// `queries`, or an alias of `queries`.
fn query_module_reaches(level: &[TokenTree]) -> Vec<String> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        if ident_at(level, at).as_deref() != Some(QUERIES) {
            continue;
        }
        if ident_at(level, at + 1).as_deref() == Some("as") {
            out.push(format!("an alias of `{QUERIES}`"));
            continue;
        }
        if !path_separator_at(level, at + 1) {
            continue;
        }
        let next = &level[(at + 3).min(level.len())..];
        match next.first() {
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                let reached = comma_separated(group.stream())
                    .into_iter()
                    .filter_map(|element| query_element_reach(&element));
                out.extend(reached);
            }
            _ => out.extend(query_element_reach(next)),
        }
    }
    out
}

/// The names `levels` give the generated commission module: `commission`, each rename of it
/// (`commission as sql`, or `commission::{self as sql}`), and each rename of those.
fn query_module_names(levels: &[Vec<TokenTree>]) -> BTreeSet<String> {
    let mut names = BTreeSet::from([COMMISSION_TABLE.to_owned()]);
    loop {
        let mut grown = names.clone();
        for level in levels {
            for at in 0..level.len() {
                let Some(ident) = ident_at(level, at) else {
                    continue;
                };
                if !names.contains(&ident) {
                    continue;
                }
                if ident_at(level, at + 1).as_deref() == Some("as")
                    && let Some(bound) = ident_at(level, at + 2)
                {
                    grown.insert(bound);
                }
                if path_separator_at(level, at + 1)
                    && let Some(TokenTree::Group(group)) = level.get(at + 3)
                {
                    for element in comma_separated(group.stream()) {
                        let renames_self = ident_at(&element, 0).as_deref() == Some("self")
                            && ident_at(&element, 1).as_deref() == Some("as");
                        if renames_self && let Some(bound) = ident_at(&element, 2) {
                            grown.insert(bound);
                        }
                    }
                }
            }
        }
        if grown == names {
            return names;
        }
        names = grown;
    }
}

/// What one path element after a commission module name names: a write function, or a glob.
fn write_fn_element(element: &[TokenTree], write_fns: &BTreeSet<String>) -> Option<String> {
    if punct_at(element, 0, '*') {
        return Some("*".to_owned());
    }
    ident_at(element, 0).filter(|name| write_fns.contains(name))
}

/// Each generated commission write function `level` names through one of `modules`: `m::f`,
/// `m::{f, …}`, or a glob `m::*`.
fn write_fn_namings(
    level: &[TokenTree],
    modules: &BTreeSet<String>,
    write_fns: &BTreeSet<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        let through_a_module = ident_at(level, at).is_some_and(|name| modules.contains(&name));
        if !through_a_module || !path_separator_at(level, at + 1) {
            continue;
        }
        let next = &level[(at + 3).min(level.len())..];
        match next.first() {
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                let named = comma_separated(group.stream())
                    .into_iter()
                    .filter_map(|element| write_fn_element(&element, write_fns));
                out.extend(named);
            }
            _ => out.extend(write_fn_element(next, write_fns)),
        }
    }
    out
}

/// Each name of `names` `level` calls or paths to as a method or an associated function:
/// after a `.` or a `::`.
fn calls_into(level: &[TokenTree], names: &BTreeSet<String>) -> Vec<String> {
    (0..level.len())
        .filter_map(|at| {
            let name = ident_at(level, at)?;
            let after_dot = at >= 1 && punct_at(level, at - 1, '.');
            let after_path = at >= 2 && path_separator_at(level, at - 2);
            let called = names.contains(&name) && (after_dot || after_path);
            called.then_some(name)
        })
        .collect()
}

/// The method name a piece in an impl stands for: the last part of its context.
fn method_of(piece: &Piece) -> &str {
    piece
        .context
        .rsplit("::")
        .next()
        .unwrap_or(piece.context.as_str())
}

/// Whether `piece` sits in the PostgreSQL write view: `impl CommissionWrites for
/// PgCommissionWrites` or an inherent impl of it.
fn in_the_pg_write_view(piece: &Piece) -> bool {
    let trait_ok = matches!(piece.trait_of.as_deref(), None | Some(COMMISSION_WRITES));
    piece.impl_of.as_deref() == Some(PG_WRITE_VIEW) && trait_ok
}

/// The inherent methods of the PostgreSQL write view in `pieces` that name a write function,
/// directly or through another of them.
fn pg_write_helpers(
    pieces: &[Piece],
    modules: &BTreeSet<String>,
    write_fns: &BTreeSet<String>,
) -> BTreeSet<String> {
    let inherent: Vec<(&str, Vec<Vec<TokenTree>>)> = pieces
        .iter()
        .filter(|piece| piece.impl_of.as_deref() == Some(PG_WRITE_VIEW))
        .filter(|piece| piece.trait_of.is_none() && piece.context.contains("::"))
        .map(|piece| (method_of(piece), levels(piece.tokens.clone())))
        .collect();
    let mut helpers: BTreeSet<String> = inherent
        .iter()
        .filter(|(_, piece_levels)| {
            piece_levels
                .iter()
                .any(|level| !write_fn_namings(level, modules, write_fns).is_empty())
        })
        .map(|(name, _)| (*name).to_owned())
        .collect();
    loop {
        let reached: BTreeSet<String> = inherent
            .iter()
            .filter(|(_, piece_levels)| {
                piece_levels
                    .iter()
                    .any(|level| !calls_into(level, &helpers).is_empty())
            })
            .map(|(name, _)| (*name).to_owned())
            .collect();
        let grown: BTreeSet<String> = helpers.union(&reached).cloned().collect();
        if grown == helpers {
            return helpers;
        }
        helpers = grown;
    }
}

/// Each struct literal or pattern of one of `names` (or `Self`, when `self_named`) in
/// `level`, a struct definition and a return type aside: a name followed by a brace group,
/// directly or past `::<…>`. Each comes with its brace group's contents.
fn struct_literals(
    level: &[TokenTree],
    names: &BTreeSet<String>,
    self_named: bool,
) -> Vec<TokenStream> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        let Some(ident) = ident_at(level, at) else {
            continue;
        };
        let named = names.contains(&ident) || (self_named && ident == "Self");
        let defined = at >= 1 && ident_at(level, at - 1).as_deref() == Some("struct");
        let returned = at >= 2 && punct_at(level, at - 1, '>') && punct_at(level, at - 2, '-');
        if !named || defined || returned {
            continue;
        }
        let mut next = at + 1;
        if path_separator_at(level, next) && punct_at(level, next + 2, '<') {
            next = past_generics(level, next + 2);
        }
        if let Some(TokenTree::Group(group)) = level.get(next)
            && group.delimiter() == Delimiter::Brace
        {
            out.push(group.stream());
        }
    }
    out
}

/// The commission write view's methods, by name, from the port file.
fn commission_write_methods() -> BTreeSet<String> {
    let port = parse(&commission_port_file());
    traits_named(&port, COMMISSION_WRITES)
        .into_iter()
        .flat_map(|tr| tr.items.iter())
        .filter(|item| matches!(item, TraitItem::Fn(_)))
        .map(trait_item_name)
        .collect()
}

/// What breaks the PostgreSQL pin in the adapter's sources `files` (each by path relative to
/// its `src`, the generated queries aside): outside the commission file, any reach into the
/// generated commission module; in it, a write function named (or a write helper or a write
/// method called) outside the write view; a write-view literal outside the unit's
/// `commissions()`; and SQL in Rust that writes a commission table or names a `.sql` file.
fn pg_pin_findings(
    files: &[(String, String)],
    write_fns: &BTreeSet<String>,
    write_methods: &BTreeSet<String>,
) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let views = token_aliases(&[PG_WRITE_VIEW], &sources);
    let mut out = Vec::new();
    for (file, source) in files {
        let shown = format!("adapter-pg/src/{file}");
        let parsed =
            syn::parse_file(source).unwrap_or_else(|error| panic!("{shown} parses: {error}"));
        let file_pieces = pieces(&parsed);
        let in_commission = file == COMMISSION_FILE;
        let file_levels: Vec<Vec<TokenTree>> = file_pieces
            .iter()
            .flat_map(|piece| levels(piece.tokens.clone()))
            .collect();
        let modules = query_module_names(&file_levels);
        let helpers = pg_write_helpers(&file_pieces, &modules, write_fns);
        let mut literals_left = usize::from(file == PG_UNIT_FILE);
        for piece in &file_pieces {
            let context = &piece.context;
            let self_named = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| views.contains(name));
            for level in levels(piece.tokens.clone()) {
                if !in_commission {
                    for reach in query_module_reaches(&level) {
                        out.push(format!("{shown}: {reach} in {context}"));
                    }
                } else if !in_the_pg_write_view(piece) {
                    for name in write_fn_namings(&level, &modules, write_fns) {
                        out.push(format!("{shown}: write query `{name}` in {context}"));
                    }
                    for name in calls_into(&level, &helpers) {
                        out.push(format!("{shown}: write helper `{name}` in {context}"));
                    }
                    if piece.impl_of.as_deref() == Some(PG_WRITE_VIEW) {
                        for name in calls_into(&level, write_methods) {
                            out.push(format!("{shown}: write method `{name}` in {context}"));
                        }
                    }
                }
                for _ in struct_literals(&level, &views, self_named) {
                    if context == PG_UNIT_COMMISSIONS && literals_left > 0 {
                        literals_left -= 1;
                    } else {
                        out.push(format!("{shown}: `{PG_WRITE_VIEW} {{ … }}` in {context}"));
                    }
                }
            }
            for text in string_literals(piece.tokens.clone()) {
                let mut findings = commission_sql_findings(&text);
                if text.to_ascii_lowercase().ends_with(".sql") {
                    findings.push("names a .sql file".to_owned());
                }
                for finding in findings {
                    out.push(format!("{shown}: SQL {text:?} {finding} in {context}"));
                }
            }
        }
    }
    out
}

/// The PostgreSQL adapter's production sources, the generated queries aside.
fn pg_sources() -> Vec<(String, String)> {
    sources_under(&adapter_pg().join("src"), &[GENERATED_QUERIES])
}

/// The generated commission write functions, read from the generated file.
fn pg_commission_write_fns() -> BTreeSet<String> {
    let generated = adapter_pg().join("src").join(GENERATED_QUERIES);
    commission_write_fns(&parse(&generated))
}

#[test]
fn every_commission_write_statement_lives_in_the_commission_folder() {
    let files = adapter_queries();

    let misplaced = misplaced_commission_sql(&files);

    assert_eq!(misplaced, Vec::<String>::new());
    let set_archived = files
        .iter()
        .find(|(file, _)| file == "commission/set_archived.sql")
        .expect("the scan reads commission/set_archived.sql");
    assert_eq!(commission_tables_written(&set_archived.1), ["commission"]);
    let changelog = files
        .iter()
        .find(|(file, _)| file == "changelog/append.sql")
        .expect("the scan reads changelog/append.sql");
    assert!(changelog.1.contains("INSERT INTO commission_changelog"));
}

#[test]
fn the_statement_scan_refuses_each_planted_write() {
    let refused = [
        "UPDATE commission SET archived_at = now() WHERE id = $1",
        "update Commission set archived_at = now()",
        "UPDATE \"commission\" SET archived_at = now()",
        "UPDATE public.commission SET archived_at = now()",
        "UPDATE public . \"commission\" SET archived_at = now()",
        "UPDATE ONLY commission SET archived_at = now()",
        "UPDATE /* the card */ commission SET archived_at = now()",
        "UPDATE -- the card\n commission SET archived_at = now()",
        "DELETE FROM commission_tab WHERE commission_id = $1",
        "INSERT INTO commission_participant (commission_id, user_id) VALUES ($1, $2)",
        "MERGE INTO commission_seat s USING workflow w ON true WHEN MATCHED THEN DELETE",
        "TRUNCATE workflow_column, commission",
        "TRUNCATE TABLE ONLY commission_slot",
        "COPY commission FROM stdin",
        "WITH gone AS (DELETE FROM commission WHERE id = $1 RETURNING id) SELECT id FROM gone",
        "UPDATE U&\"\\0063ommission\" SET archived_at = now()",
    ];
    for sql in refused {
        let files = [("column/retire_card.sql".to_owned(), sql.to_owned())];

        let found = misplaced_commission_sql(&files);

        assert!(!found.is_empty(), "{sql}");
        assert!(
            found
                .iter()
                .all(|finding| finding.starts_with("adapter-pg/queries/column/retire_card.sql")),
            "{sql}: {found:?}"
        );
    }
    let kept = [
        (
            "workflow/cards.sql",
            "SELECT w.id FROM workflow w JOIN commission c ON c.id = w.id",
        ),
        (
            "column/cards.sql",
            "SELECT c.id FROM commission c FOR UPDATE OF c",
        ),
        (
            "column/add_card.sql",
            "INSERT INTO workflow_column_commission (column_id, commission_id) VALUES ($1, $2) ON CONFLICT DO UPDATE SET position = 1",
        ),
        (
            "workflow/rename.sql",
            "UPDATE workflow SET name = c.id::text FROM commission c WHERE c.id = $1",
        ),
        ("workflow/rename.sql", "UPDATE \"Commission\" SET name = $1"),
        (
            "column/delete.sql",
            "DELETE FROM workflow_column_commission USING commission c WHERE true",
        ),
        (
            "changelog/append.sql",
            "INSERT INTO commission_changelog (commission_id) VALUES ($1)",
        ),
        (
            "commission/set_archived.sql",
            "UPDATE commission SET archived_at = $2 WHERE id = $1",
        ),
    ];
    for (file, sql) in kept {
        let files = [(file.to_owned(), sql.to_owned())];

        let found = misplaced_commission_sql(&files);

        assert_eq!(found, Vec::<String>::new(), "{file}: {sql}");
    }
}

#[test]
fn only_the_commission_write_view_calls_the_commission_write_queries() {
    let files = pg_sources();
    let write_fns = pg_commission_write_fns();
    let write_methods = commission_write_methods();

    let findings = pg_pin_findings(&files, &write_fns, &write_methods);

    assert_eq!(findings, Vec::<String>::new());
    assert!(
        write_fns.contains("set_archived") && write_fns.contains("create_commission"),
        "the generated list holds the commission writes: {write_fns:?}"
    );
    assert!(write_methods.contains("set_archived"), "{write_methods:?}");
    let read: BTreeSet<&str> = files.iter().map(|(file, _)| file.as_str()).collect();
    for file in [COMMISSION_FILE, PG_UNIT_FILE, "workflow.rs", "lib.rs"] {
        assert!(read.contains(file), "the scan reads {file}");
    }
    let (_, commission) = files
        .iter()
        .find(|(file, _)| file == COMMISSION_FILE)
        .expect("the commission file");
    let commission_pieces = pieces(&syn::parse_file(commission).expect("it parses"));
    let commission_levels: Vec<Vec<TokenTree>> = commission_pieces
        .iter()
        .flat_map(|piece| levels(piece.tokens.clone()))
        .collect();
    let modules = query_module_names(&commission_levels);
    assert!(
        modules.contains("sql"),
        "the module's alias is read: {modules:?}"
    );
    let archive = commission_pieces
        .iter()
        .find(|piece| piece.context == "PgCommissionWrites::set_archived")
        .expect("the write view's set_archived");
    let named: Vec<String> = levels(archive.tokens.clone())
        .iter()
        .flat_map(|level| write_fn_namings(level, &modules, &write_fns))
        .collect();
    assert_eq!(named, ["set_archived"]);
    let helpers = pg_write_helpers(&commission_pieces, &modules, &write_fns);
    assert!(helpers.contains("insert_element"), "{helpers:?}");
}

#[test]
fn the_pg_pin_refuses_each_planted_write() {
    let files = pg_sources();
    let write_fns = pg_commission_write_fns();
    let write_methods = commission_write_methods();
    let column_writes = "impl ColumnWrites for PgColumnWrites<'_> {";
    let store = "impl CommissionStore for PgCommissionStore {";
    let reads = "impl CommissionReads for PgCommissionWrites<'_> {";
    let retire = |body: &str| {
        format!(
            "    async fn retire_card(&mut self, commission: &CommissionId) -> anyhow::Result<bool> {{ {body} }}"
        )
    };
    let refused = [
        (
            "workflow.rs",
            column_writes,
            retire(
                "let written = crate::queries::commission::set_archived(&mut *self.conn, uuid::Uuid::from(*commission), Some(chrono::Utc::now())).await?; Ok(written == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::queries::commission as q; let written = q::set_archived(&mut *self.conn, uuid::Uuid::from(*commission), None).await?; Ok(written == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::queries::*; let written = commission::set_archived(&mut *self.conn, uuid::Uuid::from(*commission), None).await?; Ok(written == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::{queries::{column, commission::set_archived}}; Ok(set_archived(&mut *self.conn, uuid::Uuid::from(*commission), None).await? == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::queries as q; Ok(q::commission::set_archived(&mut *self.conn, uuid::Uuid::from(*commission), None).await? == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::queries::{self as q}; Ok(q::commission::delete(&mut *self.conn, uuid::Uuid::from(*commission)).await? == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let mut spent = false; CommissionWrites::set_archived(&mut crate::PgCommissionWrites { conn: &mut *self.conn, failed: &mut spent }, commission, None).await",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "type View<'a> = crate::PgCommissionWrites<'a>; let mut spent = false; let mut view = View::<'_> { conn: &mut *self.conn, failed: &mut spent }; view.set_archived(commission, None).await",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let written = sqlx::query(\"UPDATE commission SET archived_at = now() WHERE id = $1\").bind(uuid::Uuid::from(*commission)).execute(&mut *self.conn).await?; Ok(written.rows_affected() == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let written = sqlx::query(include_str!(\"../queries/commission/set_archived.sql\")).execute(&mut *self.conn).await?; Ok(written.rows_affected() == 1)",
            ),
        ),
        (
            COMMISSION_FILE,
            store,
            "    async fn retire_card(&self, id: &CommissionId) -> anyhow::Result<bool> { Ok(sql::set_archived(&self.pool, uuid::Uuid::from(*id), None).await? == 1) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            store,
            "    async fn retire_card(&self, id: &CommissionId) -> anyhow::Result<bool> { Ok(crate::queries::commission::delete(&self.pool, uuid::Uuid::from(*id)).await? == 1) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            reads,
            "    async fn retire_card(&mut self, id: &CommissionId) -> anyhow::Result<bool> { Ok(sql::set_maturity(&mut *self.conn, uuid::Uuid::from(*id), None, None).await? == 1) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            reads,
            "    async fn retire_card(&mut self, id: &CommissionId) -> anyhow::Result<()> { self.set_archived(id, None).await.map(drop) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            reads,
            "    async fn retire_card(&mut self, element: &NewElement) -> anyhow::Result<()> { self.insert_element(element).await }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            reads,
            "    async fn retire_card(&mut self) -> anyhow::Result<()> { let mut view = Self { conn: &mut *self.conn, failed: &mut *self.failed }; view.delete(&id).await }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "async fn retire(conn: &mut PgConnection, id: uuid::Uuid) -> anyhow::Result<u64> { Ok(sql::set_archived(conn, id, None).await?) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use sql as statements;\nasync fn retire(pool: &PgPool, id: uuid::Uuid) -> anyhow::Result<u64> { Ok(statements::delete(pool, id).await?) }".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use sql::set_archived as archive_row;".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use crate::queries::commission::*;".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use crate::queries::commission::{self as rows};\nfn wipe(pool: &PgPool) { let _ = rows::revoke_view; }".to_owned(),
        ),
    ];
    for (file, anchor, addition) in &refused {
        let planted = planted_after(&files, file, anchor, addition);

        let found = pg_pin_findings(&planted, &write_fns, &write_methods);

        assert!(!found.is_empty(), "{file}: {addition}");
        let shown = format!("adapter-pg/src/{file}: ");
        assert!(
            found.iter().all(|finding| finding.starts_with(&shown)),
            "{addition}: {found:?}"
        );
    }
    let kept = [
        (
            COMMISSION_FILE,
            store,
            "    async fn probe(&self, id: &CommissionId) -> anyhow::Result<bool> { Ok(sql::find(&self.pool, uuid::Uuid::from(*id)).await?.is_some()) }",
        ),
        (
            COMMISSION_FILE,
            store,
            "    async fn probe(&self, id: &CommissionId) -> anyhow::Result<()> { crate::queries::column::position_in_column(&self.pool, uuid::Uuid::from(*id), uuid::Uuid::nil()).await?; Ok(()) }",
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn probe(&mut self) -> anyhow::Result<()> { column_sql::clear_cards(&mut *self.conn, uuid::Uuid::nil()).await?; Ok(()) }",
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn probe(&mut self) -> anyhow::Result<()> { sqlx::query(\"SELECT id FROM commission WHERE id = $1\").execute(&mut *self.conn).await?; Ok(()) }",
        ),
    ];
    for (file, anchor, addition) in kept {
        let planted = planted_after(&files, file, anchor, addition);

        let found = pg_pin_findings(&planted, &write_fns, &write_methods);

        assert_eq!(found, Vec::<String>::new(), "{file}: {addition}");
    }
}

/// How a reach of the in-memory commission map (or of a column's card list) is used.
#[derive(Debug, PartialEq)]
enum MapUse {
    /// Only read.
    Read,
    /// Bound to a local by a `let`, which carries the reach on.
    Bound(String),
    /// Changed through a method other than a read, or one the scan doesn't know.
    Changed(String),
    /// Handed on: passed, returned, matched on, or assigned to or through.
    Escaped,
}

/// The first token of the place expression ending at `at`: `a.b.0.name`, past a leading `&`,
/// `&mut` or `*`.
fn place_start(level: &[TokenTree], at: usize) -> usize {
    let mut start = at;
    while start >= 2
        && punct_at(level, start - 1, '.')
        && matches!(
            level.get(start - 2),
            Some(TokenTree::Ident(_) | TokenTree::Literal(_))
        )
    {
        start -= 2;
    }
    loop {
        if start >= 1 && (punct_at(level, start - 1, '&') || punct_at(level, start - 1, '*')) {
            start -= 1;
        } else if start >= 2
            && ident_at(level, start - 1).as_deref() == Some("mut")
            && punct_at(level, start - 2, '&')
        {
            start -= 2;
        } else {
            return start;
        }
    }
}

/// The local a `let` binds to the expression from `start` to `end`: `let [mut] x [: T] = …;`.
fn bound_by_let(level: &[TokenTree], start: usize, end: usize) -> Option<String> {
    if !punct_at(level, end, ';') {
        return None;
    }
    let equals = start.checked_sub(1)?;
    if !punct_at(level, equals, '=') {
        return None;
    }
    let keyword = (0..equals)
        .rev()
        .find(|at| punct_at(level, *at, ';') || ident_at(level, *at).as_deref() == Some("let"))?;
    if ident_at(level, keyword).as_deref() != Some("let") {
        return None;
    }
    let name_at = if ident_at(level, keyword + 1).as_deref() == Some("mut") {
        keyword + 2
    } else {
        keyword + 1
    };
    let name = ident_at(level, name_at)?;
    let plain = name_at + 1 == equals || punct_at(level, name_at + 1, ':');
    plain.then_some(name)
}

/// How the reach whose place runs from `start` to `at` is used, and whether it passed a lock
/// (`locked`: it had before it was reached here).
fn map_use(level: &[TokenTree], start: usize, at: usize, locked: bool) -> (MapUse, bool) {
    let mut index = at + 1;
    let mut locked = locked;
    loop {
        if punct_at(level, index, '?') {
            index += 1;
            continue;
        }
        let method = if punct_at(level, index, '.') {
            ident_at(level, index + 1)
        } else {
            None
        };
        let Some(method) = method else {
            let bound = bound_by_let(level, start, index);
            return (bound.map_or(MapUse::Escaped, MapUse::Bound), locked);
        };
        if !parenthesized_at(level, index + 2) {
            return (MapUse::Changed(method), locked);
        }
        index += 3;
        let name = method.as_str();
        if MUTEX_LOCKS.contains(&name) {
            locked = true;
            continue;
        }
        let passes = MAP_PASSES.contains(&name) || (name == "clone" && !locked);
        if passes {
            continue;
        }
        let reach = if MAP_READS.contains(&name) {
            MapUse::Read
        } else {
            MapUse::Changed(method)
        };
        return (reach, locked);
    }
}

/// Whether the identifier at `at` is a local in use: not a field, a path's tail, a binding
/// or a field name in a struct literal or pattern.
fn local_in_use(level: &[TokenTree], at: usize) -> bool {
    let after_dot = at >= 1 && punct_at(level, at - 1, '.');
    let after_path = at >= 2 && path_separator_at(level, at - 2);
    let previous = at.checked_sub(1).and_then(|before| ident_at(level, before));
    let binds_mut = previous.as_deref() == Some("mut")
        && at >= 2
        && ident_at(level, at - 2).as_deref() == Some("let");
    let binds = previous.as_deref() == Some("let") || binds_mut;
    let field_name = punct_at(level, at + 1, ':') && !path_separator_at(level, at + 1);
    !(after_dot || after_path || binds || field_name)
}

/// Each reach in `level` of the in-memory commission map or of a local `bound` from one (with
/// whether it was bound past a lock), and whether the reach passed a lock: a `.commissions`
/// field (a method call aside), a bound local, and a bare `commissions` local reached past a
/// lock.
fn map_reaches(level: &[TokenTree], bound: &BTreeMap<String, bool>) -> Vec<(MapUse, bool)> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        let Some(ident) = ident_at(level, at) else {
            continue;
        };
        let field = ident == MEM_MAP
            && at >= 1
            && punct_at(level, at - 1, '.')
            && !parenthesized_at(level, at + 1)
            && !path_separator_at(level, at + 1);
        let local = bound
            .get(&ident)
            .copied()
            .filter(|_| local_in_use(level, at));
        if field {
            out.push(map_use(level, place_start(level, at), at, false));
        } else if let Some(locked) = local {
            out.push(map_use(level, place_start(level, at), at, locked));
        } else if ident == MEM_MAP && local_in_use(level, at) {
            let reach = map_use(level, place_start(level, at), at, false);
            if reach.1 {
                out.push(reach);
            }
        }
    }
    out
}

/// Every reach of the in-memory commission map in `piece`, locals bound from one followed.
fn piece_map_uses(piece: &Piece) -> Vec<(MapUse, bool)> {
    let piece_levels = levels(piece.tokens.clone());
    let mut bound: BTreeMap<String, bool> = BTreeMap::new();
    loop {
        let uses: Vec<(MapUse, bool)> = piece_levels
            .iter()
            .flat_map(|level| map_reaches(level, &bound))
            .collect();
        let mut grown = bound.clone();
        for (reach, locked) in &uses {
            if let MapUse::Bound(name) = reach {
                let was_locked = grown.entry(name.clone()).or_insert(false);
                *was_locked |= *locked;
            }
        }
        if grown == bound {
            return uses;
        }
        bound = grown;
    }
}

/// Whether `piece`, in the in-memory adapter's file `file`, may change the commission map:
/// a method of `impl CommissionWrites for MemCommissionWrites`, or one of [`MEM_MAP_WRITERS`].
fn changes_the_mem_map(file: &str, piece: &Piece) -> bool {
    let write_view = file == COMMISSION_FILE
        && piece.impl_of.as_deref() == Some(MEM_WRITE_VIEW)
        && piece.trait_of.as_deref() == Some(COMMISSION_WRITES);
    let named = MEM_MAP_WRITERS
        .iter()
        .any(|(writer, context)| *writer == file && piece.context == *context)
        && piece.trait_of.is_none();
    write_view || named
}

/// Whether `piece` is a `use` item: its first word, past attributes and a visibility, is `use`.
fn is_use_item(piece: &Piece) -> bool {
    let level: Vec<TokenTree> = piece.tokens.clone().into_iter().collect();
    let mut at = 0;
    loop {
        if punct_at(&level, at, '#') {
            at += 2;
        } else if ident_at(&level, at).as_deref() == Some("pub") {
            at += 1;
            if parenthesized_at(&level, at) {
                at += 1;
            }
        } else {
            return ident_at(&level, at).as_deref() == Some("use");
        }
    }
}

/// Whether the struct literal or pattern `fields` names the commission map's field.
fn names_the_map_field(fields: TokenStream) -> bool {
    comma_separated(fields)
        .iter()
        .any(|field| ident_at(field, 0).as_deref() == Some(MEM_MAP))
}

/// What breaks the in-memory pin in the adapter's sources `files` (each by path relative to
/// its `src`): the commission map changed or handed on outside [`changes_the_mem_map`]'s
/// pieces (a column's card list handed on only where [`COLUMN_CARD_LISTS`] allows); a
/// `StoredCommission` built or matched outside the insert path; a `MemBackend` literal or
/// pattern naming the map outside staging; and the write view named outside its definition,
/// its impls, a `use` and the unit's `commissions()`, which names it once. Aliases included.
fn mem_pin_findings(files: &[(String, String)]) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let rows = token_aliases(&[STORED_COMMISSION], &sources);
    let backends = token_aliases(&[MEM_BACKEND], &sources);
    let views = token_aliases(&[MEM_WRITE_VIEW], &sources);
    let mut card_lists: BTreeMap<(String, String), usize> = COLUMN_CARD_LISTS
        .iter()
        .map(|(file, context, count)| (((*file).to_owned(), (*context).to_owned()), *count))
        .collect();
    let mut out = Vec::new();
    for (file, source) in files {
        let shown = format!("adapter-mem/src/{file}");
        let parsed =
            syn::parse_file(source).unwrap_or_else(|error| panic!("{shown} parses: {error}"));
        let mut view_left = usize::from(file == MEM_LIB_FILE);
        for piece in pieces(&parsed) {
            let context = &piece.context;
            if !changes_the_mem_map(file, &piece) {
                let key = (file.clone(), context.clone());
                for (reach, locked) in piece_map_uses(&piece) {
                    let card_list = !locked && card_lists.get(&key).is_some_and(|left| *left > 0);
                    match reach {
                        MapUse::Read | MapUse::Bound(_) => {}
                        MapUse::Escaped if card_list => {
                            if let Some(left) = card_lists.get_mut(&key) {
                                *left -= 1;
                            }
                        }
                        MapUse::Escaped => {
                            out.push(format!(
                                "{shown}: the commission map handed on in {context}"
                            ));
                        }
                        MapUse::Changed(method) => {
                            out.push(format!(
                                "{shown}: the commission map changed (`{method}`) in {context}"
                            ));
                        }
                    }
                }
            }
            let in_row = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| rows.contains(name));
            let in_backend = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| backends.contains(name));
            let in_view = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| views.contains(name));
            for level in levels(piece.tokens.clone()) {
                let inserts = file == COMMISSION_FILE && context == INSERT_COMMISSION;
                for _ in struct_literals(&level, &rows, in_row) {
                    if !inserts {
                        out.push(format!(
                            "{shown}: `{STORED_COMMISSION} {{ … }}` in {context}"
                        ));
                    }
                }
                let stages = (file.as_str(), context.as_str()) == MEM_STAGE;
                for fields in struct_literals(&level, &backends, in_backend) {
                    if !stages && names_the_map_field(fields) {
                        out.push(format!(
                            "{shown}: `{MEM_BACKEND} {{ {MEM_MAP}, … }}` in {context}"
                        ));
                    }
                }
            }
            let header = in_view
                && *context == format!("impl {}", piece.impl_of.as_deref().unwrap_or_default());
            let defines = *context == format!("struct {MEM_WRITE_VIEW}");
            if header || defines || is_use_item(&piece) {
                continue;
            }
            for name in identifiers(piece.tokens.clone()) {
                if !views.contains(&name) {
                    continue;
                }
                if context == MEM_UNIT_COMMISSIONS && view_left > 0 {
                    view_left -= 1;
                } else {
                    out.push(format!("{shown}: `{name}` named in {context}"));
                }
            }
        }
    }
    out
}

/// The in-memory adapter's production sources.
fn mem_sources() -> Vec<(String, String)> {
    sources_under(&adapter_mem_src(), &[])
}

#[test]
fn only_the_commission_write_view_changes_the_in_memory_commissions() {
    let files = mem_sources();

    let findings = mem_pin_findings(&files);

    assert_eq!(findings, Vec::<String>::new());
    let (_, commission) = files
        .iter()
        .find(|(file, _)| file == COMMISSION_FILE)
        .expect("the scan reads the commission file");
    let commission_pieces = pieces(&syn::parse_file(commission).expect("it parses"));
    let uses_in = |context: &str| -> Vec<(MapUse, bool)> {
        let piece = commission_pieces
            .iter()
            .find(|piece| piece.context == context)
            .unwrap_or_else(|| panic!("the commission file holds {context}"));
        piece_map_uses(piece)
    };
    let archive = uses_in("MemCommissionWrites::set_archived");
    assert!(
        archive.contains(&(MapUse::Changed("get_mut".to_owned()), true)),
        "{archive:?}"
    );
    let delete = uses_in("MemCommissionWrites::delete");
    assert!(
        delete.contains(&(MapUse::Changed("remove".to_owned()), true)),
        "{delete:?}"
    );
    let all = uses_in("MemBackend::all_commissions");
    assert!(all.contains(&(MapUse::Read, true)), "{all:?}");
    let read: BTreeSet<&str> = files.iter().map(|(file, _)| file.as_str()).collect();
    for file in [COMMISSION_FILE, MEM_LIB_FILE, "workflow.rs"] {
        assert!(read.contains(file), "the scan reads {file}");
    }
}

#[test]
fn the_mem_pin_refuses_each_planted_change() {
    let files = mem_sources();
    let column_writes = "impl ColumnWrites for MemColumnWrites {";
    let store = "impl CommissionStore for MemCommissionStore {";
    let reads = "impl CommissionReads for MemCommissionWrites<'_> {";
    let backend = "impl MemBackend {";
    let lock = ".lock().expect(\"MemBackend commissions mutex poisoned\")";
    let retire = |body: String| {
        format!(
            "    async fn retire_card(&mut self, commission: &CommissionId) -> anyhow::Result<bool> {{ {body} }}"
        )
    };
    let refused = [
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let removed = self.0.commissions{lock}.remove(commission); Ok(removed.is_some())"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let mut commissions = self.0.commissions{lock}; let retired = commissions.get_mut(commission).is_some(); Ok(retired)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let map = &self.0.commissions; map{lock}.clear(); Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire("let shared = Arc::clone(&self.0.commissions); Ok(wipe(shared))".to_owned()),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "*self.0.commissions{lock} = HashMap::new(); Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let mut guard = self.0.commissions{lock}; *guard = HashMap::new(); Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "for (_, stored) in self.0.commissions{lock}.iter_mut() {{ retire(stored); }} Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let MemBackend {{ commissions, .. }} = self.0.clone(); commissions{lock}.clear(); Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let kept = self.0.commissions{lock}.get(commission).cloned(); self.0.commissions{lock}.insert(*commission, kept.expect(\"a row\")); Ok(true)"
            )),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "Mutex::lock(&self.0.commissions).expect(\"poisoned\").clear(); Ok(true)".to_owned(),
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let mut spent = false; let mut view = crate::commission::MemCommissionWrites(self.0.clone(), &mut spent); CommissionWrites::set_archived(&mut view, commission, None).await"
                    .to_owned(),
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::commission::StoredCommission as Row; let row: Row = Row { node_id, owner_id }; Ok(true)"
                    .to_owned(),
            ),
        ),
        (
            "workflow.rs",
            "    async fn set_commissions(&mut self, column: &Column) -> anyhow::Result<()> {",
            "wipe(Arc::clone(&self.0.commissions));".to_owned(),
        ),
        (
            COMMISSION_FILE,
            store,
            format!(
                "    async fn retire_card(&self, id: &CommissionId) -> anyhow::Result<bool> {{ Ok(self.0.commissions{lock}.get_mut(id).is_some()) }}"
            ),
        ),
        (
            COMMISSION_FILE,
            reads,
            format!(
                "    async fn retire_card(&mut self, id: &CommissionId) -> anyhow::Result<bool> {{ let mut commissions = self.0.commissions{lock}; Ok(commissions.remove(id).is_some()) }}"
            ),
        ),
        (
            COMMISSION_FILE,
            backend,
            "    fn forge(&self, node_id: NodeId, owner_id: UserId) -> StoredCommission { StoredCommission { node_id, owner_id, lifecycle_step: LifecycleStep::Active, deadline: None, maturity: None, direction_status: None, deadline_status: None, linked_channel: None, archived_at: None, created_at: chrono::Utc::now() } }".to_owned(),
        ),
        (
            MEM_LIB_FILE,
            backend,
            format!(
                "    fn reset(&self, base: &MemBackend) {{ merge_map(&mut self.commissions{lock}, &HashMap::new(), &base.commissions{lock}); }}"
            ),
        ),
        (
            MEM_LIB_FILE,
            backend,
            "    fn spend(&self) -> MemCommissionWrites<'static> { MemCommissionWrites(self.clone(), Box::leak(Box::new(false))) }".to_owned(),
        ),
    ];
    for (file, anchor, addition) in &refused {
        let planted = planted_after(&files, file, anchor, addition);

        let found = mem_pin_findings(&planted);

        assert!(!found.is_empty(), "{file}: {addition}");
        let shown = format!("adapter-mem/src/{file}: ");
        assert!(
            found.iter().all(|finding| finding.starts_with(&shown)),
            "{addition}: {found:?}"
        );
    }
    let kept = [
        (
            COMMISSION_FILE,
            store,
            format!(
                "    async fn probe(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {{ Ok(self.0.commissions{lock}.get(id).map(|stored| stored.rebuild(*id))) }}"
            ),
        ),
        (
            COMMISSION_FILE,
            store,
            format!(
                "    async fn probe(&self) -> anyhow::Result<usize> {{ let commissions = self.0.commissions{lock}; Ok(commissions.iter().filter(|(_, stored)| stored.archived_at.is_none()).count()) }}"
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn probe(&mut self, stored: &StoredColumn, id: &CommissionId) -> anyhow::Result<bool> { Ok(stored.commissions.contains(id) && !stored.commissions.is_empty()) }".to_owned(),
        ),
    ];
    for (file, anchor, addition) in &kept {
        let planted = planted_after(&files, file, anchor, addition);

        let found = mem_pin_findings(&planted);

        assert_eq!(found, Vec::<String>::new(), "{file}: {addition}");
    }
}

// `unsafe` is forbidden in every crate.

#[test]
fn every_crate_takes_the_workspace_lints() {
    let workspace = crates()
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .join("Cargo.toml");
    let root = std::fs::read_to_string(workspace).expect("the workspace manifest reads");
    let members = members_of(&metadata());

    let without: Vec<String> = members
        .iter()
        .filter(|member| {
            let manifest = std::fs::read_to_string(member.folder.join("Cargo.toml"))
                .expect("a member manifest reads");
            !manifest.contains("\n[lints]\nworkspace = true\n")
        })
        .map(|member| member.name.clone())
        .collect();

    assert!(root.contains("\n[workspace.lints.rust]\nunsafe_code = \"forbid\"\n"));
    assert_eq!(without, Vec::<String>::new());
}
