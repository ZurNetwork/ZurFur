//! The den's write gate is the only way to write a commission's node or fields. These scans
//! hold its seams: the write capability's surface, its one definition and its one issuer across
//! the workspace and the issuer's callers in the den, every act's entry rule naming one gated
//! entry written out and no two the same, the gated entries built only in the act file, each
//! with a constructor of its own (two may build one kind), each act's lent view holding its one
//! write and every act's only the read, each view write calling its own port method with the
//! capability and the stored change, the lent view's surface and its latch, nothing else in the
//! gate file reaching the lent view's fields, no compiled source the scans can't read, the write
//! host and the gate named once each, the system's viewer-less read and the sweep each with its
//! one caller, the write use case kept inside the crate, nothing of it (and no raw SQL or
//! in-memory seeder) in a driver, no lint silencer in any production file of the use-case layer,
//! every method of the commission write view taking kernel's write capability in place of the
//! commission's id or named, none taking the system's, and none with a default body, the named
//! writes clippy holds exactly the ones `application`'s clippy settings ban, the commission field
//! writes not yet gated, frozen, and the commission tables written only by the commission write
//! view in each adapter, whatever port a write is called through: in PostgreSQL, every statement
//! writing a commission table in the commission's query folder, its generated write functions
//! named only in the write view, the view built only by the unit, and no SQL text handed to sqlx
//! but the generated queries' and three named exemptions'; in memory, the commission
//! map changed only by the write view, the insert path, the lifecycle seeder and the unit's
//! staging and merge, its rows built only by the insert path, and the view built only by the
//! unit; each write statement and in-memory change named only in its own method, a capped
//! write's other parameters only named values holding no id, the banned writes' sites frozen,
//! the gated changelog constructors named only in the rows file and the act file, the rows
//! file's entry surface and entry declarations pinned, no module in it but its unit tests and
//! no inherent impl of an entry type anywhere else, no macro in a core crate, no later
//! migration's function or trigger writing a commission table or running dynamic SQL, nor its
//! view over one, body in a decoded string or change to how strings decode, and no in-memory
//! backend method or trait impl hiding a seeder.

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
const FIELD_WRITES: [&str; 3] = ["set_archived", "declare_slots", "set_linked_channel"];

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
const FROZEN_FIELD_WRITES: [(&str, &str, &str); 4] = [
    (
        "commission/archive.rs",
        "Commissions::archive",
        "set_archived",
    ),
    ("commission/delete.rs", "Commissions::delete", "delete"),
    (
        "commission/slots/declare.rs",
        "Slots::declare",
        "declare_slots",
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

// The act's entry rule.

/// A gated kind's entry: built only in [`ACT_FILE`], and named by an act only through one of
/// its associated consts.
const GATED_ENTRY: &str = "GatedEntry";

/// The den's act file, relative to the use-case layer's sources: the one file that builds a
/// [`GATED_ENTRY`].
const ACT_FILE: &str = "den/act.rs";

/// The rule a handler's [`ENTRY`] is.
const ENTRY_RULE: &str = "EntryRule";

/// A handler's entry rule.
const ENTRY: &str = "ENTRY";

/// The entry rule's variants, each carrying one gated entry.
const ENTRY_RULE_VARIANTS: [&str; 2] = ["Always", "OnChange"];

/// The handler trait, which declares [`ENTRY`] once, in [`GATE_FILE`], with no default.
const HANDLER_TRAIT: &str = "trait Handler";

/// The gated entries today's acts name, each by its file.
const NAMED_GATED_ENTRIES: [(&str, &str); 13] = [
    ("commission/create.rs", "CREATED"),
    ("commission/deadline/clear.rs", "DEADLINE_CLEARED"),
    ("commission/deadline/set.rs", "DEADLINE_SET"),
    ("commission/deadline/status/clear.rs", "DELAYED_CLEARED"),
    ("commission/deadline/status/set.rs", "DELAYED_SET"),
    ("commission/dispute/clear.rs", "DISPUTE_CLEARED"),
    ("commission/dispute/raise.rs", "DISPUTED"),
    ("commission/lifecycle/move_to.rs", "LIFECYCLE_MOVED"),
    ("commission/maturity/set.rs", "MATURITY_RATED"),
    ("commission/rename.rs", "RENAMED"),
    ("commission/status/direction/clear.rs", "STATUS_CLEARED"),
    ("commission/status/direction/set.rs", "STATUS_SET"),
    ("commission/sweep.rs", "LATE"),
];

/// The variants a gated entry's constructor is written out under: one takes no change, one
/// the act's checked change, and the system's takes the checked change and no actor.
const CONSTRUCTOR_VARIANTS: [&str; 3] = ["Bare", "Change", "System"];

/// The use-case layer's production sources, each with its path relative to its `src`.
fn application_sources() -> Vec<(String, String)> {
    sources_under(&application_src(), &[])
}

fn is_brace_group(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Group(group) if group.delimiter() == Delimiter::Brace)
}

/// Whether `tokens` hold, at any depth, an impl whose header names one of `names`.
fn impls_one_of(tokens: TokenStream, names: &BTreeSet<String>) -> bool {
    levels(tokens).iter().any(|level| {
        (0..level.len())
            .filter(|at| ident_at(level, *at).as_deref() == Some("impl"))
            .any(|at| {
                level[at + 1..]
                    .iter()
                    .take_while(|tree| !is_brace_group(tree))
                    .any(|tree| {
                        identifiers(TokenStream::from(tree.clone()))
                            .iter()
                            .any(|ident| names.contains(ident))
                    })
            })
    })
}

/// Each [`GATED_ENTRY`] struct literal or pattern in `files` (each by its path relative to the
/// use-case layer's sources) outside [`ACT_FILE`]: under any alias, or `Self` in an impl of
/// one, at any depth, function and macro bodies included.
fn gated_literal_findings(files: &[(String, String)]) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let names = token_aliases(&[GATED_ENTRY], &sources);
    let mut out = Vec::new();
    for (file, source) in files {
        if file == ACT_FILE {
            continue;
        }
        let parsed = syn::parse_file(source).expect("the source parses");
        for piece in pieces(&parsed) {
            let in_an_impl_of_one = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| names.contains(name))
                || impls_one_of(piece.tokens.clone(), &names);
            let literals: usize = levels(piece.tokens.clone())
                .iter()
                .map(|level| struct_literals(level, &names, in_an_impl_of_one).len())
                .sum();
            if literals > 0 {
                out.push(format!(
                    "{file}: a `{GATED_ENTRY}` literal in {}",
                    piece.context
                ));
            }
        }
    }
    out
}

/// The tokens of the item that starts at `at` in `level`, up to and including its `;`.
fn item_to_semicolon(level: &[TokenTree], at: usize) -> &[TokenTree] {
    let end = (at..level.len())
        .find(|index| punct_at(level, *index, ';'))
        .map_or(level.len(), |index| index + 1);
    &level[at..end]
}

/// The gated entry `value` names when it is exactly `EntryRule::Always(GatedEntry::NAME)` or
/// `EntryRule::OnChange(GatedEntry::NAME)`, as written; `None` for anything else.
fn named_gated_entry(value: &[TokenTree]) -> Option<String> {
    let variant = ident_at(value, 3)?;
    let rule_written_out = value.len() == 5
        && ident_at(value, 0).as_deref() == Some(ENTRY_RULE)
        && path_separator_at(value, 1)
        && ENTRY_RULE_VARIANTS.contains(&variant.as_str());
    let Some(TokenTree::Group(group)) = value.get(4) else {
        return None;
    };
    if !rule_written_out || group.delimiter() != Delimiter::Parenthesis {
        return None;
    }
    let inner: Vec<TokenTree> = group.stream().into_iter().collect();
    let gated_written_out = inner.len() == 4
        && ident_at(&inner, 0).as_deref() == Some(GATED_ENTRY)
        && path_separator_at(&inner, 1);
    gated_written_out.then(|| ident_at(&inner, 3)).flatten()
}

/// What one `const ENTRY` item is: the bare declaration, or the gated entry its value names
/// when written out; `Err` with the item as written otherwise.
enum EntryItem {
    Declared,
    Names(String),
}

fn entry_item(item: &[TokenTree]) -> Result<EntryItem, String> {
    let shown = || item.iter().cloned().collect::<TokenStream>().to_string();
    let typed = punct_at(item, 2, ':') && ident_at(item, 3).as_deref() == Some(ENTRY_RULE);
    let ends = item
        .last()
        .is_some_and(|tree| matches!(tree, TokenTree::Punct(punct) if punct.as_char() == ';'));
    if !typed || !ends {
        return Err(shown());
    }
    let past_type = if punct_at(item, 4, '<') {
        past_generics(item, 4)
    } else {
        4
    };
    if past_type == item.len() - 1 {
        return Ok(EntryItem::Declared);
    }
    if !punct_at(item, past_type, '=') {
        return Err(shown());
    }
    named_gated_entry(&item[past_type + 1..item.len() - 1])
        .map(EntryItem::Names)
        .ok_or_else(shown)
}

/// Every gated entry a production `const ENTRY` in `files` names, by file and piece, and what
/// breaks the entry rule's pin: a `const ENTRY` at any depth (macro bodies included) not
/// written out as `EntryRule::Always(GatedEntry::NAME)` or `EntryRule::OnChange(GatedEntry::NAME)`,
/// a bare declaration anywhere but once in [`HANDLER_TRAIT`] in [`GATE_FILE`], and a gated
/// entry two items name.
fn entry_rule_scan(files: &[(String, String)]) -> (Vec<(String, String)>, Vec<String>) {
    let mut named: Vec<(String, String)> = Vec::new();
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut declarations = 0usize;
    let mut out = Vec::new();
    for (file, source) in files {
        let parsed = syn::parse_file(source).expect("the source parses");
        for piece in pieces(&parsed) {
            for level in levels(piece.tokens.clone()) {
                for at in 0..level.len() {
                    let starts = ident_at(&level, at).as_deref() == Some("const")
                        && ident_at(&level, at + 1).as_deref() == Some(ENTRY);
                    if !starts {
                        continue;
                    }
                    let place = format!("{file}: {}", piece.context);
                    match entry_item(item_to_semicolon(&level, at)) {
                        Ok(EntryItem::Declared) => {
                            declarations += 1;
                            let in_the_trait = file == GATE_FILE && piece.context == HANDLER_TRAIT;
                            if !in_the_trait || declarations > 1 {
                                out.push(format!("{place}: a bare `{ENTRY}` declaration"));
                            }
                        }
                        Ok(EntryItem::Names(name)) => {
                            named.push((file.clone(), name.clone()));
                            by_name.entry(name).or_default().push(place);
                        }
                        Err(written) => {
                            out.push(format!(
                                "{place}: `{written}` is no gated entry written out"
                            ));
                        }
                    }
                }
            }
        }
    }
    for (name, places) in by_name {
        if places.len() > 1 {
            out.push(format!("`{GATED_ENTRY}::{name}` is named by {places:?}"));
        }
    }
    if declarations == 0 {
        out.push(format!(
            "no `{ENTRY}` declaration in {GATE_FILE}'s {HANDLER_TRAIT}"
        ));
    }
    (named, out)
}

/// What breaks the gated entries' definitions in [`ACT_FILE`]: an associated const of an
/// inherent impl of [`GATED_ENTRY`] not written out as `Self { kinds: &[ChangelogEntryKind::K,
/// …], constructor: EntryConstructor::Bare(NewChangelogEntry::c) }` (or `::Change(…)` or
/// `::System(…)`), and
/// two consts sharing a constructor. Two constructors may build one kind.
fn gated_definition_findings(act: &syn::File) -> Vec<String> {
    let mut constructors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut out = Vec::new();
    for item in every_item(&act.items) {
        let Item::Impl(implementation) = item else {
            continue;
        };
        if implementation.trait_.is_some() || type_name(&implementation.self_ty) != GATED_ENTRY {
            continue;
        }
        for impl_item in &implementation.items {
            let ImplItem::Const(constant) = impl_item else {
                continue;
            };
            let name = constant.ident.to_string();
            let Some((_, constructor)) = gated_fields(&constant.expr) else {
                out.push(format!("`{GATED_ENTRY}::{name}` is not written out"));
                continue;
            };
            constructors.entry(constructor).or_default().push(name);
        }
    }
    for (constructor, names) in constructors {
        if names.len() > 1 {
            out.push(format!(
                "the constructor `{constructor}` is shared by {names:?}"
            ));
        }
    }
    out
}

/// The variant of `prefix` that `expr` names when it is exactly `prefix::name`.
fn two_segment_path(expr: &syn::Expr, prefix: &str) -> Option<String> {
    let syn::Expr::Path(path) = expr else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let plain = path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path
            .path
            .segments
            .iter()
            .all(|segment| segment.arguments.is_none());
    match segments.as_slice() {
        [first, name] if plain && first == prefix => Some(name.clone()),
        _ => None,
    }
}

/// The kinds a gated entry's const names: `&[ChangelogEntryKind::K, …]`, one or more, each
/// written out; `None` for anything else.
fn written_out_kinds(expr: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Reference(reference) = expr else {
        return None;
    };
    let syn::Expr::Array(array) = &*reference.expr else {
        return None;
    };
    let kinds: Vec<String> = array
        .elems
        .iter()
        .map(|kind| two_segment_path(kind, "ChangelogEntryKind"))
        .collect::<Option<_>>()?;
    let plain = reference.mutability.is_none() && !kinds.is_empty();
    plain.then_some(kinds)
}

/// The domain constructor a gated entry's const names: `EntryConstructor::Bare(NewChangelogEntry::c)`
/// or `EntryConstructor::Change(…)` or `EntryConstructor::System(…)`; `None` for anything else.
fn written_out_constructor(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Call(call) = expr else {
        return None;
    };
    let variant = two_segment_path(&call.func, "EntryConstructor")?;
    let [argument] = call.args.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    let named = CONSTRUCTOR_VARIANTS.contains(&variant.as_str());
    named
        .then(|| two_segment_path(argument, "NewChangelogEntry"))
        .flatten()
}

/// The kinds and the constructor a gated entry's const is written out with:
/// `Self { kinds: &[ChangelogEntryKind::K, …], constructor: EntryConstructor::Bare(NewChangelogEntry::c) }`
/// (or `::Change(…)` or `::System(…)`), nothing else.
fn gated_fields(expr: &syn::Expr) -> Option<(Vec<String>, String)> {
    let syn::Expr::Struct(literal) = expr else {
        return None;
    };
    let fields: Vec<(String, &syn::Expr)> = literal
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(ident) => Some((ident.to_string(), &field.expr)),
            syn::Member::Unnamed(_) => None,
        })
        .collect();
    let plain = literal.path.is_ident("Self")
        && literal.rest.is_none()
        && literal.dot2_token.is_none()
        && fields.len() == 2
        && literal.fields.len() == 2;
    let [(kinds_field, kinds), (constructor_field, constructor)] = fields.as_slice() else {
        return None;
    };
    let named = kinds_field == "kinds" && constructor_field == "constructor";
    if !plain || !named {
        return None;
    }
    let kinds = written_out_kinds(kinds)?;
    let constructor = written_out_constructor(constructor)?;
    Some((kinds, constructor))
}

#[test]
fn each_act_names_one_gated_entry_and_constructors_are_unique() {
    let files = application_sources();

    let literals = gated_literal_findings(&files);
    let (named, findings) = entry_rule_scan(&files);
    let definitions = gated_definition_findings(&parse(&application_src().join(ACT_FILE)));

    assert_eq!(literals, Vec::<String>::new());
    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(definitions, Vec::<String>::new());
    let expected: Vec<(String, String)> = NAMED_GATED_ENTRIES
        .iter()
        .map(|(file, name)| ((*file).to_owned(), (*name).to_owned()))
        .collect();
    assert_eq!(named, expected, "the scan reads today's thirteen acts");
    let act_source =
        std::fs::read_to_string(application_src().join(ACT_FILE)).expect("the act file reads");
    let shares_a_constructor = act_source.replacen(
        "        constructor: EntryConstructor::Change(NewChangelogEntry::lifecycle_moved),",
        "        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),",
        1,
    );
    assert_ne!(shares_a_constructor, act_source, "the plant takes");
    let shared = gated_definition_findings(
        &syn::parse_file(&shares_a_constructor).expect("the planted act file parses"),
    );
    assert_eq!(
        shared,
        ["the constructor `renamed` is shared by [\"RENAMED\", \"LIFECYCLE_MOVED\"]"]
    );
    let shares_a_kind = act_source.replacen(
        "        kinds: &[ChangelogEntryKind::LifecycleMoved],",
        "        kinds: &[ChangelogEntryKind::LifecycleMoved, ChangelogEntryKind::Renamed],",
        1,
    );
    assert_ne!(shares_a_kind, act_source, "the plant takes");
    let kind_shared = gated_definition_findings(
        &syn::parse_file(&shares_a_kind).expect("the planted act file parses"),
    );
    assert_eq!(
        kind_shared,
        Vec::<String>::new(),
        "two constructors may build one kind"
    );
}

/// A fixture act file: a handler of `act` whose entry rule is `entry`.
fn act_with_entry(act: &str, entry: &str) -> String {
    format!(
        "impl Handler for {act} {{\n    type Role = AsOwner;\n\n    const ENTRY: EntryRule = {entry};\n\n    async fn handle(&self, lent: &mut Lent<'_, AsOwner>) -> Result<Handled, HandlerError> {{\n        let answer = lent.lent().await?;\n        Ok(Handled {{ answer }})\n    }}\n}}"
    )
}

/// A `GatedEntry` literal building an archive's entry under kind `Renamed`.
const LEAKY_LITERAL: &str = "GatedEntry { kind: ChangelogEntryKind::Renamed, constructor: |c, a, t| NewChangelogEntry::event(c, ChangelogEntryKind::Archived, a, serde_json::json!({\"title\": \"A secret sketch\"}), t) }";

#[test]
fn the_entry_rule_scans_refuse_each_planted_shape() {
    let files = application_sources();
    let rename = "commission/rename.rs";
    let closure = act_with_entry(
        "Act",
        "EntryRule::OnChange(|c, a, t| NewChangelogEntry::renamed(c, a, t).with_note(\"chosen by the act\".to_owned()))",
    );
    let local_fn = format!(
        "fn leak(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry {{ NewChangelogEntry::renamed(c, a, t) }}\n{}",
        act_with_entry("Act", "EntryRule::OnChange(leak)")
    );
    let literal_entry = act_with_entry("Act", &format!("EntryRule::OnChange({LEAKY_LITERAL})"));
    let aliased = format!(
        "use crate::den::act::GatedEntry as Gated;\n{}",
        act_with_entry("Act", "EntryRule::OnChange(Gated::RENAMED)")
    );
    let shared = act_with_entry("Act", "EntryRule::OnChange(GatedEntry::CREATED)");
    let borrowed = act_with_entry("Act", "<rename::Act as Handler>::ENTRY");
    let another_const = format!(
        "const LEAK: EntryRule = EntryRule::OnChange(GatedEntry::RENAMED);\n{}",
        act_with_entry("Act", "LEAK")
    );
    let qualified = act_with_entry(
        "Act",
        "crate::den::act::EntryRule::OnChange(GatedEntry::RENAMED)",
    );
    let stamped = "macro_rules! act {\n    ($entry:expr) => {\n        impl Handler for Act {\n            type Role = AsOwner;\n            const ENTRY: EntryRule = $entry;\n        }\n    };\n}\nact!(EntryRule::OnChange(GatedEntry::RENAMED));";
    let stamped_name = "macro_rules! act {\n    ($name:ident) => {\n        impl Handler for Act {\n            const ENTRY: EntryRule = EntryRule::OnChange(GatedEntry::$name);\n        }\n    };\n}\nact!(RENAMED);";
    let second_trait = "pub(crate) trait Other {\n    const ENTRY: EntryRule;\n}";
    let defaulted = "pub(crate) trait Handler {\n    const ENTRY: EntryRule = EntryRule::Always(GatedEntry::CREATED);\n}";
    let entry_plants: Vec<Vec<(String, String)>> = vec![
        with_file(&files, rename, &closure),
        with_file(&files, rename, &local_fn),
        with_file(&files, "commission/archive.rs", &literal_entry),
        with_file(&files, rename, &aliased),
        with_file(&files, rename, &shared),
        with_file(&files, "commission/archive.rs", &borrowed),
        with_file(&files, rename, &another_const),
        with_file(&files, rename, &qualified),
        with_file(&files, rename, stamped),
        with_file(&files, rename, stamped_name),
        with_file(&files, "den/write.rs", second_trait),
        with_file(&files, "den/write.rs", defaulted),
    ];
    for planted in &entry_plants {
        let (_, findings) = entry_rule_scan(planted);

        assert!(!findings.is_empty(), "{:?}", planted.last());
    }
    let (_, shared_findings) = entry_rule_scan(&with_file(&files, rename, &shared));
    assert!(
        shared_findings
            .iter()
            .any(|finding| finding.contains("GatedEntry::CREATED")),
        "{shared_findings:?}"
    );

    let in_a_fn = format!("fn leak() -> GatedEntry {{ {LEAKY_LITERAL} }}");
    let in_a_const = format!("const LEAK: GatedEntry = {LEAKY_LITERAL};");
    let through_an_alias = format!(
        "use crate::den::act::GatedEntry as Gated;\nfn leak() -> Gated {{ {} }}",
        LEAKY_LITERAL.replacen(GATED_ENTRY, "Gated", 1)
    );
    let through_a_type = format!(
        "type Gated = GatedEntry;\nfn leak() -> Gated {{ {} }}",
        LEAKY_LITERAL.replacen(GATED_ENTRY, "Gated", 1)
    );
    let as_self = format!(
        "impl GatedEntry {{\n    const LEAK: Self = {};\n}}",
        LEAKY_LITERAL.replacen(GATED_ENTRY, "Self", 1)
    );
    let nested_impl = format!(
        "fn hide() {{\n    impl GatedEntry {{\n        const LEAK: Self = {};\n    }}\n}}",
        LEAKY_LITERAL.replacen(GATED_ENTRY, "Self", 1)
    );
    let in_a_macro = "macro_rules! gated {\n    ($constructor:expr) => {\n        $crate::den::act::GatedEntry { kind: ChangelogEntryKind::Renamed, constructor: $constructor }\n    };\n}";
    let in_a_pattern = "fn take(rule: EntryRule) { let EntryRule::Always(GatedEntry { kind, .. }) = rule else { return; }; }";
    let literal_plants: Vec<Vec<(String, String)>> = vec![
        with_file(&files, "commission/archive.rs", &in_a_const),
        with_file(&files, "den/write.rs", &in_a_fn),
        with_file(&files, "den/write.rs", &through_an_alias),
        with_file(&files, "den/write.rs", &through_a_type),
        with_file(&files, "den/write.rs", &as_self),
        with_file(&files, "den/write.rs", &nested_impl),
        with_file(&files, "den/write.rs", in_a_macro),
        with_file(&files, "den/write.rs", in_a_pattern),
    ];
    for planted in &literal_plants {
        let findings = gated_literal_findings(planted);

        assert!(!findings.is_empty(), "{:?}", planted.last());
    }
    let kept_self = "impl Command {\n    fn fresh(actor: UserId, act: DenAct) -> Self {\n        Self { actor, act }\n    }\n}";
    let kept = with_file(&files, "den/write.rs", kept_self);
    assert_eq!(gated_literal_findings(&kept), Vec::<String>::new());
    let named_in_write = with_file(
        &files,
        "den/write.rs",
        "fn rule() -> EntryRule { EntryRule::Always(GatedEntry::CREATED) }",
    );
    assert_eq!(
        gated_literal_findings(&named_in_write),
        Vec::<String>::new()
    );

    let act_source =
        std::fs::read_to_string(application_src().join(ACT_FILE)).expect("the act file reads");
    let definition_plants = [
        (
            "        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),",
            "        constructor: EntryConstructor::Bare(NewChangelogEntry::created),",
        ),
        (
            "    pub(crate) const RENAMED: Self = Self {",
            "    pub(crate) const ALSO_CREATED: Self = Self::CREATED;\n\n    pub(crate) const RENAMED: Self = Self {",
        ),
        (
            "        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),",
            "        constructor: EntryConstructor::Bare(|c, a, t| NewChangelogEntry::renamed(c, a, t)),",
        ),
        (
            "        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),",
            "        constructor: EntryConstructor::Leaky(NewChangelogEntry::renamed),",
        ),
        (
            "        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),",
            "        constructor: NewChangelogEntry::renamed,",
        ),
        (
            "        kinds: &[ChangelogEntryKind::Renamed],",
            "        kinds: &[],",
        ),
        (
            "        kinds: &[ChangelogEntryKind::Renamed],",
            "        kinds: RENAMED_KINDS,",
        ),
        (
            "        kinds: &[ChangelogEntryKind::Renamed],",
            "        kinds: &[ChangelogEntryKind::Renamed, crate::Kind::Created],",
        ),
        (
            "impl GatedEntry<LifecycleMove> {",
            "impl GatedEntry<LifecycleMove> {\n    pub(crate) const MOVED_AGAIN: Self = Self {\n        kinds: &[ChangelogEntryKind::Renamed],\n        constructor: EntryConstructor::Change(NewChangelogEntry::lifecycle_moved),\n    };\n",
        ),
    ];
    for (from, to) in definition_plants {
        assert!(act_source.contains(from), "{from}");
        let planted = act_source.replacen(from, to, 1);
        let act = syn::parse_file(&planted).expect("the planted act file parses");

        let findings = gated_definition_findings(&act);

        assert!(!findings.is_empty(), "{to}");
    }
}

/// Each item of `act`, the act file, that holds an `impl` of [`GATED_ENTRY`] (or an alias of
/// it) below the top level: in a module, a function or const body, another impl's items or a
/// macro. Only a top-level impl's consts are read as gated entries.
fn nested_gated_impls(act: &syn::File) -> Vec<String> {
    let names = token_aliases(&[GATED_ENTRY], &[act.to_token_stream().to_string()]);
    act.items
        .iter()
        .filter_map(|item| {
            let inner: TokenStream = match item {
                Item::Impl(implementation) => implementation
                    .items
                    .iter()
                    .map(|impl_item| impl_item.to_token_stream())
                    .collect(),
                other => other.to_token_stream(),
            };
            impls_one_of(inner, &names).then(|| item_name(item))
        })
        .collect()
}

#[test]
fn gated_entries_are_defined_only_in_top_level_impls() {
    let act_source =
        std::fs::read_to_string(application_src().join(ACT_FILE)).expect("the act file reads");
    let act = syn::parse_file(&act_source).expect("the act file parses");

    let nested = nested_gated_impls(&act);

    assert_eq!(nested, Vec::<String>::new());
    let leak = "const LEAK: Self = Self { kinds: &[ChangelogEntryKind::Renamed], constructor: EntryConstructor::Bare(NewChangelogEntry::renamed) };";
    let refused = [
        format!("mod hidden {{\n    use super::*;\n    impl GatedEntry<LateMark> {{ {leak} }}\n}}"),
        format!("fn hide() {{\n    impl GatedEntry<LateMark> {{ {leak} }}\n}}"),
        format!("const _: () = {{\n    impl GatedEntry<LateMark> {{ {leak} }}\n}};"),
        format!("macro_rules! gated {{\n    () => {{ impl GatedEntry<LateMark> {{ {leak} }} }};\n}}"),
        format!("impl Handled {{\n    fn hide() {{ impl GatedEntry<LateMark> {{ {leak} }} }}\n}}"),
        format!("use self::GatedEntry as Gated;\nmod hidden {{\n    impl super::Gated<LateMark> {{ {leak} }}\n}}"),
        "mod hidden {\n    impl Default for super::GatedEntry<LateMark> { fn default() -> Self { todo!() } }\n}".to_owned(),
    ];
    for addition in &refused {
        let planted = format!("{act_source}\n{addition}\n");
        let file = syn::parse_file(&planted).expect("the planted act file parses");

        let found = nested_gated_impls(&file);

        assert_eq!(found.len(), 1, "{addition}: {found:?}");
    }
    let top_level = format!("{act_source}\nimpl GatedEntry<LateMark> {{ {leak} }}\n");
    let kept = syn::parse_file(&top_level).expect("the planted act file parses");
    assert_eq!(nested_gated_impls(&kept), Vec::<String>::new());
}

// No core crate defines a macro.

/// The crates whose production sources define no macro: a macro's expansion is a write no
/// scan here can read.
const MACRO_FREE_CRATES: [&str; 8] = [
    "application",
    "workflow",
    "kernel",
    "identity",
    "domain",
    "atta",
    "adapter-pg",
    "adapter-mem",
];

/// Each macro definition in `tokens`, at any depth: a `macro_rules!`, a `macro` item, and a
/// `#[macro_export]` or `#[macro_use]` attribute, under `cfg_attr` too.
fn macro_definitions(tokens: TokenStream) -> Vec<String> {
    let mut out = Vec::new();
    for level in levels(tokens) {
        for at in 0..level.len() {
            let Some(ident) = ident_at(&level, at) else {
                continue;
            };
            let rules = ident == "macro_rules" && punct_at(&level, at + 1, '!');
            let declared = ident == "macro" && ident_at(&level, at + 1).is_some();
            let attribute = ident == "macro_export" || ident == "macro_use";
            if rules || declared || attribute {
                out.push(ident);
            }
        }
    }
    out
}

/// Each macro definition in the production sources under `src`, by file as `shown` names its
/// folder.
fn macro_findings_under(src: &Path, shown: &str) -> Vec<String> {
    walk(src)
        .production
        .iter()
        .flat_map(|path| {
            let file = relative(path, src);
            macro_definitions(parse(path).to_token_stream())
                .into_iter()
                .map(move |definition| format!("{shown}/{file}: `{definition}`"))
        })
        .collect()
}

#[test]
fn no_core_crate_defines_a_macro() {
    let found: Vec<String> = MACRO_FREE_CRATES
        .iter()
        .flat_map(|krate| {
            let shown = format!("{krate}/src");
            macro_findings_under(&crates().join(krate).join("src"), &shown)
        })
        .collect();

    assert_eq!(found, Vec::<String>::new());
    let test_support = macro_findings_under(&crates().join("test-support/src"), "test-support/src");
    assert!(
        !test_support.is_empty(),
        "the scan reads test-support's macros"
    );
    let defined_in_lib = fixture_tree(
        "macro_defined_in_lib",
        &[
            (
                "lib.rs",
                "#[macro_export]\nmacro_rules! archive_raw {\n    ($uow:expr, $id:expr) => {\n        $uow.commissions().set_archived(&$id, None)\n    };\n}\n\nmod commission;",
            ),
            ("commission.rs", "mod archive;"),
            (
                "commission/archive.rs",
                "async fn archive(uow: Unit<'_>, id: CommissionId) { archive_raw!(uow, id).await; }",
            ),
        ],
    );
    let refused_in_lib = macro_findings_under(&defined_in_lib, "application/src");
    assert_eq!(
        refused_in_lib,
        [
            "application/src/lib.rs: `macro_export`",
            "application/src/lib.rs: `macro_rules`",
        ]
    );
    let refused = [
        "fn hide() { macro_rules! inner { () => {}; } }",
        "#[cfg_attr(feature = \"dev-profile\", macro_export)]\nfn stamp() {}",
        "#[macro_use]\nextern crate test_support;",
        "mod inner { pub macro stamp($id:expr) { $id } }",
        "impl Den { fn hide() { macro_rules! inner { () => {}; } } }",
    ];
    for source in refused {
        let tree = fixture_tree(
            "macro_refused",
            &[("lib.rs", "mod commission;"), ("commission.rs", source)],
        );

        let found_here = macro_findings_under(&tree, "application/src");

        assert!(
            found_here
                .iter()
                .all(|finding| finding.starts_with("application/src/commission.rs: ")),
            "{source}: {found_here:?}"
        );
        assert!(!found_here.is_empty(), "{source}");
    }
    let kept = fixture_tree(
        "macro_kept",
        &[
            ("lib.rs", "mod commission;\n#[cfg(test)]\nmod tests;"),
            (
                "commission.rs",
                "fn calls() -> Vec<String> { let name = \"macro_rules!\"; vec![format!(\"{name}\")] }\nfn macro_like() -> bool { matches!(1, 1) }",
            ),
            ("tests.rs", "macro_rules! in_a_test { () => {}; }"),
        ],
    );
    assert_eq!(
        macro_findings_under(&kept, "application/src"),
        Vec::<String>::new()
    );
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

/// What a generic impl on the lent view or its inner view is for: every act.
const EVERY_ACT: &str = "every act";

/// The one method a generic impl on the lent view or its inner view may hold: the read.
const LENT_READ: &str = "lent";

/// Each act with a write, as the gate file writes its type, and the one write its view holds,
/// which calls the commission repo's method of the same name and nothing else.
const ACT_WRITES: [(&str, &str); 11] = [
    ("create::Act", "create"),
    ("move_to::Act", "set_lifecycle"),
    ("dispute::raise::Act", "set_dispute"),
    ("dispute::clear::Act", "clear_dispute"),
    ("direction::set::Act", "set_direction_status"),
    ("direction::clear::Act", "set_direction_status"),
    ("deadline::set::Act", "set_deadline"),
    ("deadline::clear::Act", "set_deadline"),
    ("deadline::status::set::Act", "set_deadline_status"),
    ("deadline::status::clear::Act", "set_deadline_status"),
    ("maturity::set::Act", "set_maturity"),
];

/// The modules the gate file must import each act's module from, so `create::Act`,
/// `move_to::Act`, `direction::set::Act` and `deadline::set::Act` name the acts they say.
const ACT_MODULE_HOMES: [(&str, &str); 6] = [
    ("create", "crate::commission"),
    ("move_to", "crate::commission::lifecycle"),
    ("dispute", "crate::commission"),
    ("direction", "crate::commission::status"),
    ("deadline", "crate::commission"),
    ("maturity", "crate::commission"),
];

/// A type as written, its path's segments joined by `::`, generic arguments left out:
/// `create::Act` for `create::Act`; the type's tokens for anything but a plain path.
fn type_path(typ: &Type) -> String {
    let Type::Path(path) = typ else {
        return typ.to_token_stream().to_string();
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return typ.to_token_stream().to_string();
    }
    path.path
        .segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<String>>()
        .join("::")
}

/// The act an inherent impl on the lent view (or its inner view) is for: its self type's act
/// argument as written, or [`EVERY_ACT`] when that argument is the impl's own type parameter.
fn lent_role(implementation: &syn::ItemImpl) -> String {
    let Type::Path(path) = &*implementation.self_ty else {
        return "an unnamed type".to_owned();
    };
    let Some(last) = path.path.segments.last() else {
        return "an unnamed type".to_owned();
    };
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return "no act".to_owned();
    };
    let parameters: BTreeSet<String> = implementation
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect();
    let act = arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(act) => Some(type_path(act)),
        _ => None,
    });
    match act {
        Some(act) if parameters.contains(&act) => EVERY_ACT.to_owned(),
        Some(act) => act,
        None => "no act".to_owned(),
    }
}

/// Each method of an inherent impl on one of `views` (the lent view or its inner view, under any
/// alias) in `file`, inline modules included, with the act its impl is for.
fn view_methods(file: &syn::File, views: &BTreeSet<String>) -> Vec<(String, String)> {
    every_item(&file.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Impl(implementation)
                if views.contains(&type_name(&implementation.self_ty))
                    && implementation.trait_.is_none() =>
            {
                Some(implementation)
            }
            _ => None,
        })
        .flat_map(|implementation| {
            let act = lent_role(implementation);
            implementation
                .items
                .iter()
                .filter_map(move |impl_item| match impl_item {
                    ImplItem::Fn(method) => Some((method.sig.ident.to_string(), act.clone())),
                    _ => None,
                })
        })
        .collect()
}

/// What breaks the one-write-per-act pin for the methods `methods` (each with the act its impl
/// is for): a generic impl holding anything but [`LENT_READ`], an act's impl holding anything
/// but its one write from [`ACT_WRITES`], an impl for an act the table doesn't name, and a
/// table act whose write is missing or held twice.
fn one_write_findings(methods: &[(String, String)], view: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut held: BTreeMap<&str, usize> = BTreeMap::new();
    for (method, act) in methods {
        if act == EVERY_ACT {
            if method != LENT_READ {
                out.push(format!("a generic `{view}` impl holds `{method}`"));
            }
            continue;
        }
        let Some((table_act, write)) = ACT_WRITES.iter().find(|(table_act, _)| table_act == act)
        else {
            out.push(format!(
                "`{view}` holds `{method}` for an act with no write: {act}"
            ));
            continue;
        };
        if method != write {
            out.push(format!(
                "`{view}<{act}>` holds `{method}`, not its write `{write}`"
            ));
            continue;
        }
        *held.entry(table_act).or_default() += 1;
    }
    for (act, write) in ACT_WRITES {
        let count = held.get(act).copied().unwrap_or_default();
        if count != 1 {
            out.push(format!(
                "`{view}<{act}>` holds its write `{write}` {count} times"
            ));
        }
    }
    let reads = methods
        .iter()
        .filter(|(method, act)| act == EVERY_ACT && method == LENT_READ)
        .count();
    if reads != 1 {
        out.push(format!(
            "`{view}` holds `{LENT_READ}` for every act {reads} times"
        ));
    }
    out
}

/// What breaks the gate file's imports of the acts' modules: each of [`ACT_MODULE_HOMES`] bound
/// once, by a `use` of its home.
fn act_module_findings(file: &syn::File) -> Vec<String> {
    let imported = imports(file);
    ACT_MODULE_HOMES
        .iter()
        .filter_map(|(module, home)| {
            let from: Vec<&str> = imported
                .iter()
                .filter(|(name, _)| name == module)
                .map(|(_, path)| path.as_str())
                .collect();
            (from != [*home]).then(|| {
                format!("the gate file imports `{module}` from {from:?}, not once from `{home}`")
            })
        })
        .collect()
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

/// What breaks the lent view's pinned surface in `file`: each act's view holds its one write
/// from [`ACT_WRITES`], every act's holds `lent`, and nothing else; and the acts' modules are
/// imported from their homes.
fn lent_surface_findings(file: &syn::File) -> Vec<String> {
    let (methods, mut out) = lent_surface(file);
    out.extend(one_write_findings(&methods, LENT));
    out.extend(act_module_findings(file));
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
fn each_lent_view_holds_its_act_s_one_write() {
    let gate = parse(&application_src().join(GATE_FILE));

    let findings = lent_surface_findings(&gate);
    let elsewhere = lent_impls_elsewhere();

    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(elsewhere, Vec::<String>::new());
    let (methods, _) = lent_surface(&gate);
    let expected: Vec<(String, String)> = [
        ("create", "create::Act"),
        ("set_lifecycle", "move_to::Act"),
        ("set_dispute", "dispute::raise::Act"),
        ("clear_dispute", "dispute::clear::Act"),
        ("set_direction_status", "direction::set::Act"),
        ("set_direction_status", "direction::clear::Act"),
        ("set_deadline", "deadline::set::Act"),
        ("set_deadline", "deadline::clear::Act"),
        ("set_deadline_status", "deadline::status::set::Act"),
        ("set_deadline_status", "deadline::status::clear::Act"),
        ("set_maturity", "maturity::set::Act"),
        (LENT_READ, EVERY_ACT),
    ]
    .iter()
    .map(|(method, act)| ((*method).to_owned(), (*act).to_owned()))
    .collect();
    assert_eq!(methods, expected, "the scan reads every lent method");
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let widened = format!("{WIDENED_MODULE}{AFTER_THE_LENT_VIEW}");
    let broken = [
        (
            "    view: View<'a, H>,",
            "    pub(crate) view: View<'a, H>,",
        ),
        (
            "pub(crate) struct Lent<'a, H: Handler> {",
            "#[derive(Default)]\npub(crate) struct Lent<'a, H: Handler> {",
        ),
        (
            "impl Lent<'_, create::Act> {",
            "impl Lent<'_, rename::Act> {",
        ),
        (
            "impl Lent<'_, move_to::Act> {",
            "impl Lent<'_, create::Act> {",
        ),
        (
            "impl Lent<'_, create::Act> {",
            "impl<H: Handler> std::ops::Deref for Lent<'_, H> {\n    type Target = Box<dyn CommissionRepo>;\n    fn deref(&self) -> &Self::Target {\n        &self.view.repo\n    }\n}\n\nimpl Lent<'_, create::Act> {",
        ),
        (
            "impl Lent<'_, create::Act> {",
            "impl Lent<'_, rename::Act> {\n    pub(crate) async fn create(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.create().await)\n    }\n}\n\nimpl Lent<'_, create::Act> {",
        ),
        (
            "impl Lent<'_, move_to::Act> {",
            "impl Lent<'_, move_to::Act> {\n    pub(crate) async fn set_deadline(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.set_deadline().await)\n    }\n",
        ),
        ("impl Lent<'_, move_to::Act> {", "impl Lent<'_, Move> {"),
        (
            "    commission::{\n        act::CommissionAct, create, deadline, dispute, lifecycle::move_to, maturity, rename,\n        status::direction,\n    },",
            "    commission::{\n        act::CommissionAct, create as move_to, deadline, dispute, lifecycle::move_to as create,\n        maturity, rename, status::direction,\n    },",
        ),
        (
            "    commission::{\n        act::CommissionAct, create, deadline, dispute, lifecycle::move_to, maturity, rename,\n        status::direction,\n    },",
            "    commission::{\n        act::CommissionAct, create, deadline as direction, dispute, lifecycle::move_to, maturity,\n        rename, status::direction as deadline,\n    },",
        ),
        (
            "impl Lent<'_, direction::set::Act> {",
            "impl Lent<'_, direction::clear::Act> {",
        ),
        (
            "impl Lent<'_, direction::set::Act> {",
            "impl Lent<'_, direction::set::Act> {\n    pub(crate) async fn set_deadline(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.set_deadline().await)\n    }\n",
        ),
        (AFTER_THE_LENT_VIEW, widened.as_str()),
        (
            AFTER_THE_LENT_VIEW,
            "mod outer {\n    mod inner {\n        impl super::super::Lent<'_, rename::Act> {\n            pub(crate) async fn grant(&mut self) -> anyhow::Result<()> {\n                self.latch.pass(self.view.grant().await)\n            }\n        }\n    }\n}\n\n/// What a lent handler answered",
        ),
        (
            AFTER_THE_LENT_VIEW,
            "type Owned<'a> = Lent<'a, rename::Act>;\n\nimpl Owned<'_> {\n    pub(crate) async fn grant(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.grant().await)\n    }\n}\n\n/// What a lent handler answered",
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

/// A generic impl on the lent view that writes: every act's view would hold the write.
const GENERIC_LENT_WRITE: &str = "impl<H: Handler> Lent<'_, H> {\n    pub(crate) async fn set_lifecycle(&mut self) -> anyhow::Result<()> {\n        self.latch.pass(self.view.set_lifecycle().await)\n    }\n}\n\n";

/// The same on the inner view.
const GENERIC_VIEW_WRITE: &str = "impl<H: Handler> View<'_, H> {\n    async fn set_lifecycle(&mut self) -> anyhow::Result<()> {\n        let commission = <H::Role as RoleTable>::commission(self.capability.thing());\n        Ok(())\n    }\n}\n\n";

#[test]
fn a_generic_lent_impl_holds_lent_only() {
    let gate = parse(&application_src().join(GATE_FILE));
    let views = aliases_in_file(&[VIEW_TYPE], &gate);
    let lents = aliases_in_file(&[LENT], &gate);

    let generic_views: Vec<(String, String)> = view_methods(&gate, &views)
        .into_iter()
        .filter(|(_, act)| act == EVERY_ACT)
        .collect();
    let generic_lents: Vec<(String, String)> = view_methods(&gate, &lents)
        .into_iter()
        .filter(|(_, act)| act == EVERY_ACT)
        .collect();

    let read = [(LENT_READ.to_owned(), EVERY_ACT.to_owned())];
    assert_eq!(generic_views, read);
    assert_eq!(generic_lents, read);
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let planted_items = [
        GENERIC_LENT_WRITE,
        GENERIC_VIEW_WRITE,
        "impl<A: Handler> Lent<'_, A> {\n    pub(crate) async fn lent_again(&mut self) -> anyhow::Result<Commission> {\n        self.latch.pass(self.view.lent_again().await)\n    }\n}\n\n",
    ];
    for item in planted_items {
        let planted = source.replacen(
            AFTER_THE_LENT_VIEW,
            &format!("{item}{AFTER_THE_LENT_VIEW}"),
            1,
        );
        let file = syn::parse_file(&planted).expect("the planted gate parses");
        let views = aliases_in_file(&[VIEW_TYPE], &file);
        let lents = aliases_in_file(&[LENT], &file);

        let findings: Vec<String> = one_write_findings(&view_methods(&file, &views), VIEW_TYPE)
            .into_iter()
            .chain(one_write_findings(&view_methods(&file, &lents), LENT))
            .collect();

        assert!(
            findings
                .iter()
                .any(|finding| finding.starts_with("a generic")),
            "{item}: {findings:?}"
        );
    }
}

/// Each repo method `tokens` call on `self.repo`, with its argument tokens, at any depth.
fn repo_writes(tokens: TokenStream) -> Vec<(String, TokenStream)> {
    levels(tokens)
        .iter()
        .flat_map(|level| {
            (0..level.len()).filter_map(move |at| {
                let on_the_repo = ident_at(level, at).as_deref() == Some("self")
                    && punct_at(level, at + 1, '.')
                    && ident_at(level, at + 2).as_deref() == Some(REPO)
                    && punct_at(level, at + 3, '.');
                let method = ident_at(level, at + 4).filter(|_| on_the_repo)?;
                let Some(TokenTree::Group(arguments)) = level.get(at + 5) else {
                    return None;
                };
                (arguments.delimiter() == Delimiter::Parenthesis)
                    .then(|| (method, arguments.stream()))
            })
        })
        .collect()
}

/// Whether `argument` is the change bound as `change`: the binding itself, or the port's value
/// built from it by `From`, written `&Value::from(change.clone())`.
fn is_the_change(argument: &[TokenTree], change: &str) -> bool {
    if argument.len() == 1 && ident_at(argument, 0).as_deref() == Some(change) {
        return true;
    }
    let built_by_from = argument.len() == 6
        && punct_at(argument, 0, '&')
        && ident_at(argument, 1).is_some()
        && path_separator_at(argument, 2)
        && ident_at(argument, 4).as_deref() == Some("from");
    let Some(TokenTree::Group(group)) = argument.get(5) else {
        return false;
    };
    let inner: Vec<TokenTree> = group.stream().into_iter().collect();
    let cloned = matches!(
        inner.get(3),
        Some(TokenTree::Group(call)) if call.delimiter() == Delimiter::Parenthesis && call.stream().is_empty()
    );
    let of_the_change = inner.len() == 4
        && ident_at(&inner, 0).as_deref() == Some(change)
        && punct_at(&inner, 1, '.')
        && ident_at(&inner, 2).as_deref() == Some("clone")
        && cloned;
    built_by_from && group.delimiter() == Delimiter::Parenthesis && of_the_change
}

/// Whether `tokens`, an argument list, passes `&self.capability` and the change bound from
/// `self.change`, which the method's body binds as `let Some(name) = &self.change`: as it is,
/// or as the port's value built from it by `From`.
fn passes_the_capability_and_the_change(arguments: &TokenStream, change: &str) -> bool {
    let arguments: Vec<Vec<TokenTree>> = comma_separated(arguments.clone());
    let capability = arguments.first().is_some_and(|first| {
        punct_at(first, 0, '&')
            && ident_at(first, 1).as_deref() == Some("self")
            && punct_at(first, 2, '.')
            && ident_at(first, 3).as_deref() == Some("capability")
            && first.len() == 4
    });
    let change_passed = arguments
        .last()
        .is_some_and(|last| is_the_change(last, change));
    capability && change_passed
}

/// The name a method binds from its view's checked change: `let Some(name) = &self.change`.
fn change_binding(tokens: TokenStream) -> Option<String> {
    levels(tokens).iter().find_map(|level| {
        (0..level.len()).find_map(|at| {
            let bound = ident_at(level, at).as_deref() == Some("let")
                && ident_at(level, at + 1).as_deref() == Some("Some");
            let Some(TokenTree::Group(group)) = level.get(at + 2) else {
                return None;
            };
            let name: Vec<TokenTree> = group.stream().into_iter().collect();
            let from_the_change = punct_at(level, at + 3, '=')
                && punct_at(level, at + 4, '&')
                && ident_at(level, at + 5).as_deref() == Some("self")
                && punct_at(level, at + 6, '.')
                && ident_at(level, at + 7).as_deref() == Some("change");
            let single = name.len() == 1;
            (bound && from_the_change && single)
                .then(|| ident_at(&name, 0))
                .flatten()
        })
    })
}

/// What breaks each act's view's write in `file`: the inner view's impls hold the same one
/// write per act as the lent view's ([`ACT_WRITES`]), and each act's write calls exactly the
/// commission repo's method of its own name, once, with `&self.capability` first and the change
/// it bound from `self.change` last; the generic read calls `find` alone.
fn view_write_findings(file: &syn::File) -> Vec<String> {
    let views = aliases_in_file(&[VIEW_TYPE], file);
    let mut out = one_write_findings(&view_methods(file, &views), VIEW_TYPE);
    for item in every_item(&file.items) {
        let Item::Impl(implementation) = item else {
            continue;
        };
        if !views.contains(&type_name(&implementation.self_ty)) || implementation.trait_.is_some() {
            continue;
        }
        let act = lent_role(implementation);
        for impl_item in &implementation.items {
            let ImplItem::Fn(method) = impl_item else {
                continue;
            };
            let name = method.sig.ident.to_string();
            let body = method.block.to_token_stream();
            let calls = repo_writes(body.clone());
            let called: Vec<&str> = calls.iter().map(|(called, _)| called.as_str()).collect();
            if act == EVERY_ACT {
                if called != ["find"] {
                    out.push(format!(
                        "`{VIEW_TYPE}::{name}` for every act calls {called:?}"
                    ));
                }
                continue;
            }
            if called != [name.as_str()] {
                out.push(format!(
                    "`{VIEW_TYPE}<{act}>::{name}` calls {called:?}, not `{name}` once"
                ));
                continue;
            }
            let passes = change_binding(body).is_some_and(|change| {
                calls
                    .iter()
                    .all(|(_, arguments)| passes_the_capability_and_the_change(arguments, &change))
            });
            if !passes {
                out.push(format!(
                    "`{VIEW_TYPE}<{act}>::{name}` doesn't write the capability and the stored change"
                ));
            }
        }
    }
    out
}

#[test]
fn each_view_write_calls_its_own_port_method() {
    let gate = parse(&application_src().join(GATE_FILE));

    let findings = view_write_findings(&gate);

    assert_eq!(findings, Vec::<String>::new());
    let views = aliases_in_file(&[VIEW_TYPE], &gate);
    let writes: Vec<(String, String)> = view_methods(&gate, &views);
    assert_eq!(
        writes.len(),
        12,
        "the scan reads every view method: {writes:?}"
    );
    let source =
        std::fs::read_to_string(application_src().join(GATE_FILE)).expect("the gate reads");
    let move_call = "self.repo.set_lifecycle(&self.capability, change)";
    let raise_call = "self.repo.set_dispute(&self.capability, change)";
    let clear_call = "self.repo.clear_dispute(&self.capability, change)";
    let rate_call = "self.repo.set_maturity(&self.capability, change)";
    let create_call = ".create(&self.capability, &birth, commission)";
    let direction_call =
        ".set_direction_status(&self.capability, &DirectionChange::from(change.clone()))";
    let broken = [
        (
            direction_call,
            ".set_deadline(&self.capability, &DirectionChange::from(change.clone()))",
        ),
        (
            direction_call,
            ".set_direction_status(&self.capability, &DirectionChange::from(other.clone()))",
        ),
        (
            direction_call,
            ".set_direction_status(&self.capability, &DirectionChange::from(change))",
        ),
        (
            direction_call,
            ".set_direction_status(&self.capability, &DirectionChange::forged(change.clone()))",
        ),
        (
            ".set_deadline(&self.capability, &DeadlineChange::from(change.clone()))",
            ".set_deadline(&forged, &DeadlineChange::from(change.clone()))",
        ),
        (
            ".set_deadline_status(&self.capability, &DelayedChange::from(change.clone()))",
            ".set_deadline_status(&self.capability, &DelayedChange::from(change.clone()))\n            .await?;\n        self.repo\n            .set_deadline(&self.capability, &DeadlineChange::from(change.clone()))",
        ),
        (
            "impl View<'_, direction::set::Act> {",
            "impl View<'_, direction::set::Act> {\n    async fn set_deadline(&mut self) -> anyhow::Result<()> {\n        Ok(())\n    }\n",
        ),
        (
            move_call,
            "self.repo.set_deadline(&self.capability, change)",
        ),
        (
            move_call,
            "self.repo.set_lifecycle(&self.capability, change).await?;\n        self.repo.set_deadline_status(&self.capability, change)",
        ),
        (
            move_call,
            "self.repo.set_lifecycle(&self.capability, &other)",
        ),
        (move_call, "self.repo.set_lifecycle(&forged, change)"),
        (
            "        let Some(change) = &self.change else {\n            self.wrote = Some(Wrote::Nothing);\n            return Ok(());\n        };\n        self.wrote = Some(Wrote::Missed);\n        let landed",
            "        let change = &forged;\n        self.wrote = Some(Wrote::Missed);\n        let landed",
        ),
        (create_call, ".set_lifecycle(&self.capability, commission)"),
        (
            raise_call,
            "self.repo.set_lifecycle(&self.capability, change)",
        ),
        (
            clear_call,
            "self.repo.set_dispute(&self.capability, change)",
        ),
        (
            rate_call,
            "self.repo.set_direction_status(&self.capability, change)",
        ),
        (rate_call, "self.repo.set_maturity(&forged, change)"),
        (
            "impl View<'_, move_to::Act> {",
            "impl View<'_, rename::Act> {",
        ),
        (
            "impl View<'_, move_to::Act> {",
            "impl View<'_, move_to::Act> {\n    async fn set_deadline(&mut self) -> anyhow::Result<()> {\n        Ok(())\n    }\n",
        ),
        (
            "            .find(&commission)",
            "            .set_lifecycle(&commission)",
        ),
        (
            AFTER_THE_LENT_VIEW,
            &format!("{GENERIC_VIEW_WRITE}{AFTER_THE_LENT_VIEW}"),
        ),
    ];
    for (from, to) in broken {
        assert!(source.contains(from), "{from}");
        let planted = source.replacen(from, to, 1);
        let file = syn::parse_file(&planted).expect("the planted gate parses");

        let findings = view_write_findings(&file);

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
    assert_eq!(methods.len(), 12, "the scan reads all twelve lent methods");
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
            "        self.latch.pass(self.view.create().await)",
            "        self.view.create().await",
        ),
        (
            "        self.latch.pass(self.view.set_lifecycle().await)",
            "        self.view.set_lifecycle().await",
        ),
        (
            "        self.latch.pass(self.view.set_direction_status().await)",
            "        self.view.set_direction_status().await",
        ),
        (
            "        self.latch.pass(self.view.set_deadline().await)",
            "        self.view.set_deadline().await",
        ),
        (
            "        self.latch.pass(self.view.set_deadline_status().await)",
            "        self.view.set_deadline_status().await",
        ),
        (
            "        self.latch.pass(self.view.lent().await)",
            "        self.latch.pass(self.view.create().await)",
        ),
        (
            "        self.latch.pass(self.view.set_lifecycle().await)",
            "        self.latch.pass(self.view.set_lifecycle().await);\n        self.latch.pass(self.view.set_lifecycle().await)",
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

/// The gate's settling of a lent view after its handler: it takes the lent view and its inner
/// view apart.
const SETTLE: &str = "settle";

/// The struct literals and patterns of the views [`LEND_AS`] and [`SETTLE`] hold: the lender
/// builds the inner view and the lent view, and the settling takes each apart.
const LENDER_STRUCTS: [(&str, &str, usize); 4] = [
    (LEND_AS, VIEW_TYPE, 1),
    (LEND_AS, LENT, 1),
    (SETTLE, LENT, 1),
    (SETTLE, VIEW_TYPE, 1),
];

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

/// The name and brace contents of the struct literal or pattern the identifier at `at` opens,
/// when it is one of `names` (or `Self`, when `self_named`): a brace group next, or after a
/// turbofish. A definition (`struct`, `enum`, `union`, `impl`) and a return type are none.
fn struct_literal_at(
    level: &[TokenTree],
    at: usize,
    names: &BTreeSet<String>,
    self_named: bool,
) -> Option<(String, TokenStream)> {
    let name = ident_at(level, at)?;
    let named = names.contains(&name) || (self_named && name == "Self");
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
    let Some(TokenTree::Group(braces)) = level.get(braces_at) else {
        return None;
    };
    let braced = braces.delimiter() == Delimiter::Brace;
    (named && braced && !defines && !returned).then(|| (name, braces.stream()))
}

/// Each view of `views` a struct literal or pattern in `piece` names, in order.
fn view_structs(piece: &Piece, views: &BTreeSet<String>) -> Vec<String> {
    let held = held_by_a_view(piece, views);
    levels(piece.tokens.clone())
        .iter()
        .flat_map(|level| {
            (0..level.len()).filter_map(|at| struct_literal_at(level, at, views, held))
        })
        .map(|(name, _)| name)
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
    let held_by = |context: &str| -> Vec<String> {
        pieces(&gate)
            .iter()
            .filter(|piece| piece.context == context)
            .flat_map(|piece| view_structs(piece, &views))
            .collect()
    };
    assert_eq!(
        held_by(LEND_AS),
        [VIEW_TYPE, LENT],
        "the scan reads the view structs {LEND_AS} holds"
    );
    assert_eq!(
        held_by(SETTLE),
        [LENT, VIEW_TYPE],
        "the scan reads the view structs {SETTLE} holds"
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
    let taken_apart_early = [
        (
            "    let handled = handler.handle(&mut lent).await;\n",
            "    let handled = handler.handle(&mut lent).await;\n    let Lent { view, .. } = &mut lent;\n",
        ),
        (
            "    drop(repo);\n",
            "    drop(repo);\n    let View { node, .. } = View { ..todo!() };\n",
        ),
    ];
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
    for (from, to) in taken_apart_early {
        assert!(source.contains(from), "{from}");
        planted.push(source.replacen(from, to, 1));
    }
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

// The system's act: its viewer-less read and its sweep each have one caller.

/// The commission unit's locked read that reads no viewer: the system's grant alone calls it.
const SYSTEM_LOCK: &str = "lock_for_system";

/// The deadline sweep's use case: the leader's pass alone calls it.
const SWEEP_USE_CASE: &str = "sweep_deadlines";

/// The production sources the one-caller pins read: the use-case layer's and every driver's,
/// each by its path from the crates folder.
fn caller_scan_files() -> Vec<(String, String)> {
    let application = walk(&application_src()).production.into_iter();
    let drivers = driver_files().into_iter().map(|(_, _, path)| path);
    application
        .chain(drivers)
        .map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            (relative(&path, &crates()), source)
        })
        .collect()
}

/// Every naming of `seed`, or of an alias of it, in `files`, by file, piece and name.
fn caller_namings(files: &[(String, String)], seed: &str) -> Vec<Naming> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let names = with_aliases(&[seed], &sources);
    let mut out = Vec::new();
    for (file, source) in files {
        let parsed = syn::parse_file(source).expect("the source parses");
        out.extend(named_past(file, &pieces(&parsed), &names, &[]));
    }
    out.sort();
    out
}

/// The one naming of the viewer-less read: in the system's grant.
fn the_system_grant_s_call() -> Vec<Naming> {
    vec![(
        "application/src/den/authority.rs".to_owned(),
        "system_grant".to_owned(),
        SYSTEM_LOCK.to_owned(),
    )]
}

/// The two namings of the sweep: its definition, and the leader's pass.
fn the_sweep_s_namings() -> Vec<Naming> {
    vec![
        (
            SWEEP.to_owned(),
            LEADER_LOCK.to_owned(),
            SWEEP_USE_CASE.to_owned(),
        ),
        (
            "application/src/commission/sweep.rs".to_owned(),
            "Commissions::sweep_deadlines".to_owned(),
            SWEEP_USE_CASE.to_owned(),
        ),
    ]
}

#[test]
fn the_viewer_less_read_is_called_only_by_the_system_grant() {
    let files = caller_scan_files();

    let namings = caller_namings(&files, SYSTEM_LOCK);

    assert_eq!(namings, the_system_grant_s_call());
    let authority = "application/src/den/authority.rs";
    let system_arm = "async fn system_grant(unit: Unit<'_>, commission: CommissionId) { let locked = unit.commissions().lock_for_system(&commission).await; }";
    let user_arm = "async fn user_grant(unit: Unit<'_>, commission: CommissionId, user: &UserId) { let locked = unit.commissions().lock_for_system(&commission).await; let standing = unit.commissions().standing_for_update(&commission, user).await; }";
    let kept = [(authority.to_owned(), system_arm.to_owned())];
    assert_eq!(
        caller_namings(&kept, SYSTEM_LOCK),
        the_system_grant_s_call()
    );
    let planted = [
        (authority, format!("{system_arm}\n{user_arm}")),
        (
            authority,
            "async fn system_grant(unit: Unit<'_>, commission: CommissionId) { unit.commissions().lock_for_system(&commission).await; unit.commissions().lock_for_system(&commission).await; }".to_owned(),
        ),
        (
            "application/src/commission/den.rs",
            "async fn verdict(unit: Unit<'_>, commission: CommissionId) { let read = <dyn CommissionReads>::lock_for_system; }".to_owned(),
        ),
        (
            "cli/src/commands/commission.rs",
            "async fn sweep(unit: Unit<'_>, commission: CommissionId) { tokio::join!(async { unit.commissions().lock_for_system(&commission).await }); }".to_owned(),
        ),
    ];
    for (file, source) in planted {
        let fixture = [
            (authority.to_owned(), system_arm.to_owned()),
            (file.to_owned(), source.clone()),
        ];
        let fixture = if file == authority {
            vec![(file.to_owned(), source.clone())]
        } else {
            fixture.to_vec()
        };

        let found = caller_namings(&fixture, SYSTEM_LOCK);

        assert_ne!(found, the_system_grant_s_call(), "{file}: {source}");
    }
}

#[test]
fn sweep_deadlines_is_called_only_by_the_leader() {
    let files = caller_scan_files();

    let namings = caller_namings(&files, SWEEP_USE_CASE);

    assert_eq!(namings, the_sweep_s_namings());
    let definition = (
        "application/src/commission/sweep.rs".to_owned(),
        "impl Commissions<'_> { #[use_case] pub async fn sweep_deadlines(&self, #[ports] ports: &Ports, now: DateTimeUtc) -> CommissionResult<SweepResult> { todo!() } }".to_owned(),
    );
    let leader = (
        SWEEP.to_owned(),
        "async fn sweep_pass_as_leader(runtime: &AppState) -> anyhow::Result<Option<usize>> { let swept = runtime.app().commissions().sweep_deadlines(Utc::now()).await?; Ok(Some(swept.marked_late)) }".to_owned(),
    );
    let kept = [definition.clone(), leader.clone()];
    assert_eq!(caller_namings(&kept, SWEEP_USE_CASE), the_sweep_s_namings());
    let planted = [
        (
            "api/src/routes/commissions/sweep.rs",
            "async fn sweep(State(state): State<AppState>) -> StatusCode { let later = Utc::now() + chrono::Duration::days(365); let _ = state.app().commissions().sweep_deadlines(later).await; StatusCode::NO_CONTENT }",
        ),
        (
            SWEEP,
            "async fn run_deadline_sweeper(runtime: AppState) { let _ = runtime.app().commissions().sweep_deadlines(Utc::now()).await; }",
        ),
        (
            "cli/src/commands/sweep.rs",
            "fn sweeper() { let sweep = Commissions::sweep_deadlines; }",
        ),
        (
            "application/src/commission/delete.rs",
            "impl Commissions<'_> { async fn delete(&self) { let _ = self.sweep_deadlines(Utc::now()).await; } }",
        ),
    ];
    for (file, source) in planted {
        let fixture = [
            definition.clone(),
            leader.clone(),
            (file.to_owned(), source.to_owned()),
        ];

        let found = caller_namings(&fixture, SWEEP_USE_CASE);

        assert_ne!(found, the_sweep_s_namings(), "{file}: {source}");
    }
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

/// The in-memory adapter's seeders, each a write beneath the gate, derived from its sources
/// (see [`in_memory_seeders`]). The dev profile reaches them only through the fixture.
const IN_MEMORY_SEEDERS: InMemorySeeders = InMemorySeeders;

/// The derived seeder list, read once and handed out by name.
struct InMemorySeeders;

impl IntoIterator for InMemorySeeders {
    type Item = &'static str;
    type IntoIter = std::vec::IntoIter<&'static str>;

    fn into_iter(self) -> Self::IntoIter {
        static SEEDERS: OnceLock<BTreeSet<String>> = OnceLock::new();
        let seeders = SEEDERS.get_or_init(|| in_memory_seeders(&mem_sources()));
        let names: Vec<&'static str> = seeders.iter().map(String::as_str).collect();
        names.into_iter()
    }
}

/// Every item in `items` at any depth: inline modules, and function, method and const bodies.
fn items_at_any_depth(items: &[Item]) -> Vec<&Item> {
    let mut out = Vec::new();
    for item in items {
        out.push(item);
        let nested: Vec<&Item> = match item {
            Item::Mod(module) => module
                .content
                .iter()
                .flat_map(|(_, content)| content.iter())
                .collect(),
            Item::Fn(function) => block_items(&function.block),
            Item::Const(constant) => match constant.expr.as_ref() {
                syn::Expr::Block(body) => block_items(&body.block),
                _ => Vec::new(),
            },
            Item::Impl(implementation) => implementation
                .items
                .iter()
                .flat_map(|impl_item| match impl_item {
                    ImplItem::Fn(method) => block_items(&method.block),
                    _ => Vec::new(),
                })
                .collect(),
            _ => Vec::new(),
        };
        for inner in nested {
            out.extend(items_at_any_depth(std::slice::from_ref(inner)));
        }
    }
    out
}

/// The items a block declares among its statements.
fn block_items(block: &syn::Block) -> Vec<&Item> {
    block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

/// Today's inherent `MemBackend` helpers that carry a port method's name, each a seed or
/// inspect helper delegating to its port's in-memory view. A token scan can't tell a driver's
/// call of one from the port call of the same name, so no other inherent method may carry a
/// port method's name ([`backend_surface_findings`]); each leaves this list when it is renamed.
const PORT_NAMED_HELPERS: [&str; 7] = [
    "create",
    "create_invitation",
    "find",
    "find_by_did",
    "find_pending_invitation",
    "provision",
    "role_of",
];

/// The traits `MemBackend` may carry, by its derive or an impl: the ones it derives today.
const BACKEND_TRAITS: [&str; 2] = ["Clone", "Default"];

/// One `pub` method of an inherent impl of `MemBackend`: the file it sits in, its name, and
/// whether it is the boot's ([`boots_the_ports`]).
struct BackendMethod {
    file: String,
    name: String,
    boots: bool,
}

/// Every impl in `files`, at any depth (see [`items_at_any_depth`]), with its file.
fn impls_in(files: &[(String, String)]) -> Vec<(String, syn::ItemImpl)> {
    files
        .iter()
        .flat_map(|(file, source)| {
            let parsed = syn::parse_file(source).expect("the source parses");
            items_at_any_depth(&parsed.items)
                .into_iter()
                .filter_map(|item| match item {
                    Item::Impl(implementation) => Some((file.clone(), implementation.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The names `MemBackend` goes by in `files`: its own, and each alias at any depth.
fn backend_names(files: &[(String, String)]) -> BTreeSet<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    token_aliases(&[MEM_BACKEND], &sources)
}

/// Every `pub` method of an inherent impl of `MemBackend` in `files` (an alias, a module or a
/// body included).
fn backend_methods(files: &[(String, String)]) -> Vec<BackendMethod> {
    let backends = backend_names(files);
    impls_in(files)
        .into_iter()
        .filter(|(_, implementation)| {
            implementation.trait_.is_none()
                && backends.contains(&type_name(&implementation.self_ty))
        })
        .flat_map(|(file, implementation)| {
            implementation
                .items
                .iter()
                .filter_map(|impl_item| match impl_item {
                    ImplItem::Fn(method) if public(&method.vis) => Some(BackendMethod {
                        file: file.clone(),
                        name: unraw(&method.sig.ident),
                        boots: boots_the_ports(&method.sig),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The port method names in `files`: every method of every trait impl.
fn port_method_names(files: &[(String, String)]) -> BTreeSet<String> {
    impls_in(files)
        .iter()
        .filter(|(_, implementation)| implementation.trait_.is_some())
        .flat_map(|(_, implementation)| &implementation.items)
        .filter_map(|impl_item| match impl_item {
            ImplItem::Fn(method) => Some(unraw(&method.sig.ident)),
            _ => None,
        })
        .collect()
}

/// The in-memory adapter's seeders a driver may not name, from its sources `files`: every
/// `pub` method of an inherent impl of `MemBackend` (an alias, a module or a body included),
/// but the boot the dev profile builds its ports with (`new` and each port handle, `&self` in,
/// `Arc<dyn …>` out) and today's [`PORT_NAMED_HELPERS`].
fn in_memory_seeders(files: &[(String, String)]) -> BTreeSet<String> {
    backend_methods(files)
        .into_iter()
        .filter(|method| !method.boots && !PORT_NAMED_HELPERS.contains(&method.name.as_str()))
        .map(|method| method.name)
        .collect()
}

/// The inherent `MemBackend` methods in `files` that carry a port method's name, the boot's
/// aside, by name.
fn port_named_backend_methods(files: &[(String, String)]) -> BTreeSet<String> {
    let port_methods = port_method_names(files);
    backend_methods(files)
        .into_iter()
        .filter(|method| !method.boots && port_methods.contains(&method.name))
        .map(|method| method.name)
        .collect()
}

/// Whether `implementation`'s self type is one of its own generic parameters: a blanket impl.
fn is_a_blanket_impl(implementation: &syn::ItemImpl) -> bool {
    let Type::Path(path) = implementation.self_ty.as_ref() else {
        return false;
    };
    let parameters: BTreeSet<String> = implementation
        .generics
        .type_params()
        .map(|parameter| parameter.ident.to_string())
        .collect();
    path.qself.is_none()
        && path.path.segments.len() == 1
        && parameters.contains(&path.path.segments[0].ident.to_string())
}

/// The traits `MemBackend`'s definitions in `files` derive, by last name.
fn backend_derives(files: &[(String, String)]) -> Vec<(String, String)> {
    let backends = backend_names(files);
    files
        .iter()
        .flat_map(|(file, source)| {
            let parsed = syn::parse_file(source).expect("the source parses");
            items_at_any_depth(&parsed.items)
                .into_iter()
                .filter_map(|item| match item {
                    Item::Struct(structure) if backends.contains(&structure.ident.to_string()) => {
                        Some(structure.attrs.clone())
                    }
                    _ => None,
                })
                .flatten()
                .filter(|attribute| attribute.path().is_ident("derive"))
                .flat_map(|attribute| identifiers(attribute.meta.to_token_stream()))
                .filter(|name| name != "derive")
                .map(|name| (file.clone(), name))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// What lets a driver reach a raw in-memory write past the derived seeders, in adapter-mem's
/// sources `files`: an inherent `MemBackend` method carrying a port method's name outside
/// [`PORT_NAMED_HELPERS`]; a trait impl for a type naming `MemBackend` (an alias, a reference
/// or a wrapper included) or a blanket impl, unless of one of [`BACKEND_TRAITS`]; and a derive
/// on `MemBackend` outside them.
fn backend_surface_findings(files: &[(String, String)]) -> Vec<String> {
    let backends = backend_names(files);
    let port_named = port_named_backend_methods(files);
    let mut out: Vec<String> = backend_methods(files)
        .into_iter()
        .filter(|method| port_named.contains(&method.name))
        .filter(|method| !PORT_NAMED_HELPERS.contains(&method.name.as_str()))
        .map(|method| {
            format!(
                "adapter-mem/src/{}: `{MEM_BACKEND}::{}` carries a port method's name",
                method.file, method.name
            )
        })
        .collect();
    for (file, implementation) in impls_in(files) {
        let Some((_, trait_path, _)) = &implementation.trait_ else {
            continue;
        };
        let trait_name = trait_path
            .segments
            .last()
            .map(|segment| unraw(&segment.ident))
            .unwrap_or_default();
        let names_the_backend = identifiers(implementation.self_ty.to_token_stream())
            .iter()
            .any(|name| backends.contains(name));
        let reaches = names_the_backend || is_a_blanket_impl(&implementation);
        if reaches && !BACKEND_TRAITS.contains(&trait_name.as_str()) {
            let self_type = implementation.self_ty.to_token_stream();
            let trait_impl = format!("adapter-mem/src/{file}: `impl {trait_name} for {self_type}`");
            out.push(trait_impl);
        }
    }
    for (file, derived) in backend_derives(files) {
        if !BACKEND_TRAITS.contains(&derived.as_str()) {
            let derive = format!("adapter-mem/src/{file}: `{MEM_BACKEND}` derives `{derived}`");
            out.push(derive);
        }
    }
    out
}

/// Whether `signature` is the boot's: `new()`, or a port handle taking `&self` alone and
/// answering `Arc<dyn …>`.
fn boots_the_ports(signature: &syn::Signature) -> bool {
    let constructor = signature.ident == "new" && signature.inputs.is_empty();
    let only_self = matches!(
        signature.inputs.iter().collect::<Vec<_>>().as_slice(),
        [syn::FnArg::Receiver(receiver)] if receiver.reference.is_some() && receiver.mutability.is_none()
    );
    let handle = match &signature.output {
        syn::ReturnType::Type(_, ty) => {
            let answered = ty.to_token_stream().to_string().replace(' ', "");
            answered.starts_with("Arc<dyn") && answered.ends_with('>')
        }
        syn::ReturnType::Default => false,
    };
    constructor || (only_self && handle && signature.asyncness.is_none())
}

/// The private store's migration doors past the custody migrator: a driver migrates only
/// through `CustodyMigrator`, which the scan's whole-identifier match lets pass.
const MIGRATION_DOORS: [&str; 4] = ["migrate_fresh", "migrator", "Migrator", "migrate_reporting"];

/// What a driver may not name: the node port, the issuer, the capability, the in-memory
/// seeders, raw SQL, and a migration door past the custody migrator.
fn driver_forbidden(sources: &[String]) -> BTreeSet<String> {
    let seeds: Vec<&str> = NODE_PORT
        .iter()
        .copied()
        .chain([GATE_ISSUED, WRITE_CAPABILITY])
        .chain(IN_MEMORY_SEEDERS)
        .chain(RAW_SQL)
        .chain(MIGRATION_DOORS)
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
fn every_in_memory_seeder_is_derived_and_refused_in_a_driver() {
    let files = mem_sources();

    let seeders = in_memory_seeders(&files);

    let named_before = [
        "seed_bound_commission",
        "seed_stored_title",
        "seed_lifecycle_step",
        "seed_tab",
        "seed_participant",
        "remove_participant",
        "seed_changelog_entry",
    ];
    for seeder in named_before {
        assert!(seeders.contains(seeder), "{seeder}: {seeders:?}");
    }
    let boot_and_ports = [
        "new",
        "database",
        "user_store",
        "profile_cache",
        "create",
        "find",
        "provision",
    ];
    for name in boot_and_ports {
        assert!(!seeders.contains(name), "{name}: {seeders:?}");
    }
    let handed_out: BTreeSet<String> = IN_MEMORY_SEEDERS.into_iter().map(str::to_owned).collect();
    assert_eq!(handed_out, seeders);
    let derived_from = [
        (
            "dispute.rs",
            "use crate::MemBackend;\nimpl MemBackend {\n    pub fn seed_dispute(&self) {}\n    pub(crate) fn stage_dispute(&self) {}\n    pub fn dispute_store(&self) -> Arc<dyn DisputeStore> { todo!() }\n}",
            "seed_dispute",
        ),
        (
            "slot.rs",
            "type Backend = crate::MemBackend;\nimpl Backend {\n    pub async fn seed_slot_owner(&self, slot: ElementId) {}\n}",
            "seed_slot_owner",
        ),
        (
            "hidden.rs",
            "fn hide() {\n    impl crate::MemBackend {\n        pub fn seed_hidden(&self) {}\n    }\n}",
            "seed_hidden",
        ),
        (
            "store.rs",
            "mod inner {\n    impl crate::MemBackend {\n        pub fn seed_store(&self, row: StoredRow) {}\n    }\n}",
            "seed_store",
        ),
    ];
    for (file, source, seeder) in derived_from {
        let planted = with_file(&files, file, source);

        let derived = in_memory_seeders(&planted);

        assert!(derived.contains(seeder), "{file}: {derived:?}");
        assert!(!derived.contains("stage_dispute") && !derived.contains("dispute_store"));
    }
    let forbidden = driver_forbidden(&[]);
    let dev_profile_file = crates().join("api/src/dev_profile.rs");
    let dev_profile = std::fs::read_to_string(&dev_profile_file).expect("the dev profile reads");
    let boot = "    seed_commission(&backend, &alice(), &bob(), &supreme_arts()).await?;\n";
    assert!(
        dev_profile.contains(boot),
        "the dev profile boots the fixture"
    );
    let seeds_a_row = format!(
        "{boot}    backend.seed_changelog_entry(adapter_mem::fixture::untitled(), ChangelogEntryKind::Created, None, serde_json::json!({{}}), chrono::Utc::now());\n"
    );
    let planted = dev_profile.replacen(boot, &seeds_a_row, 1);
    let tree = fixture_tree(
        "dev_profile_seeds_a_row",
        &[
            (
                "lib.rs",
                "#[cfg(feature = \"dev-profile\")]\npub mod dev_profile;",
            ),
            ("dev_profile.rs", &planted),
        ],
    );
    let found: Vec<String> = walk(&tree)
        .production
        .iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(path).expect("a fixture source");
            let file = format!("api/src/{}", relative(path, &tree));
            driver_findings(API, &file, &source, &forbidden)
        })
        .collect();
    assert_eq!(
        found,
        ["api/src/dev_profile.rs: `seed_changelog_entry` in runtime"]
    );
    let today = driver_findings(API, "api/src/dev_profile.rs", &dev_profile, &forbidden);
    assert_eq!(today, Vec::<String>::new());
}

#[test]
fn no_backend_method_or_trait_impl_carries_a_seeder_past_the_derived_list() {
    let files = mem_sources();

    let findings = backend_surface_findings(&files);

    assert_eq!(findings, Vec::<String>::new());
    let port_named = port_named_backend_methods(&files);
    let listed: BTreeSet<String> = PORT_NAMED_HELPERS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        port_named, listed,
        "the port-named helpers are today's, exactly"
    );
    let derives: Vec<String> = backend_derives(&files)
        .into_iter()
        .map(|(_, derived)| derived)
        .collect();
    assert_eq!(derives, BACKEND_TRAITS, "the backend's derives are read");
    let append = "use crate::MemBackend;\nimpl MemBackend {\n    pub fn append(&self, commission: CommissionId, kind: ChangelogEntryKind, payload: Value, at: DateTimeUtc) {}\n}";
    let refused = [
        (
            "forge.rs",
            append,
            "adapter-mem/src/forge.rs: `MemBackend::append` carries a port method's name",
        ),
        (
            "forge.rs",
            "type Store = crate::MemBackend;\nimpl Store {\n    pub fn set_archived(&self, commission: CommissionId) {}\n}",
            "adapter-mem/src/forge.rs: `MemBackend::set_archived` carries a port method's name",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self);\n}\nimpl Forge for crate::MemBackend {\n    fn forge(&self) {}\n}",
            "adapter-mem/src/forge.rs: `impl Forge for crate :: MemBackend`",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self);\n}\ntype Store = crate::MemBackend;\nimpl Forge for Store {\n    fn forge(&self) {}\n}",
            "adapter-mem/src/forge.rs: `impl Forge for Store`",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self);\n}\nimpl Forge for std::sync::Arc<crate::MemBackend> {\n    fn forge(&self) {}\n}",
            "adapter-mem/src/forge.rs: `impl Forge for std :: sync :: Arc < crate :: MemBackend >`",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self);\n}\nfn hide() {\n    impl Forge for &crate::MemBackend {\n        fn forge(&self) {}\n    }\n}",
            "adapter-mem/src/forge.rs: `impl Forge for & crate :: MemBackend`",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self) {}\n}\nimpl<T: Clone> Forge for T {}",
            "adapter-mem/src/forge.rs: `impl Forge for T`",
        ),
    ];
    for (file, source, expected) in refused {
        let planted = with_file(&files, file, source);

        let found = backend_surface_findings(&planted);

        assert!(
            found.iter().any(|finding| finding == expected),
            "{source}: {found:?}"
        );
    }
    let (lib, lib_source) = files
        .iter()
        .find(|(file, _)| file == MEM_LIB_FILE)
        .expect("the crate root");
    let derive = "#[derive(Clone, Default)]\npub struct MemBackend {";
    assert!(
        lib_source.contains(derive),
        "{lib} derives the backend's traits"
    );
    let wider = lib_source.replacen(
        derive,
        "#[derive(Clone, Default, derive_more::Constructor)]\npub struct MemBackend {",
        1,
    );
    let found = backend_surface_findings(&with_file(&files, lib, &wider));
    assert!(
        found.contains(&"adapter-mem/src/lib.rs: `MemBackend` derives `Constructor`".to_owned()),
        "{found:?}"
    );
    let planted = with_file(&files, "forge.rs", append);
    let seeders = in_memory_seeders(&planted);
    assert!(seeders.contains("append"), "{seeders:?}");
    let seeder_names: Vec<&str> = seeders.iter().map(String::as_str).collect();
    let forbidden = with_aliases(&seeder_names, &[]);
    let herons_driver = "pub fn forge(backend: &adapter_mem::MemBackend, commission: CommissionId) {\n    backend.append(commission, ChangelogEntryKind::Late, serde_json::json!({}), chrono::Utc::now());\n}";
    let found = driver_findings(API, "api/src/heron_plant.rs", herons_driver, &forbidden);
    assert_eq!(found, ["api/src/heron_plant.rs: `append` in forge"]);
    let kept = [
        (
            "forge.rs",
            "use crate::MemBackend;\nimpl MemBackend {\n    pub(crate) fn append(&self) {}\n}",
        ),
        (
            "forge.rs",
            "pub trait Forge {\n    fn forge(&self);\n}\nimpl Forge for crate::MemNodeStore {\n    fn forge(&self) {}\n}",
        ),
        (
            "forge.rs",
            "struct Wrapper<T>(T);\nimpl<T: Clone> Clone for Wrapper<T> {\n    fn clone(&self) -> Self { Self(self.0.clone()) }\n}",
        ),
    ];
    for (file, source) in kept {
        let planted = with_file(&files, file, source);

        let found = backend_surface_findings(&planted);

        assert_eq!(found, Vec::<String>::new(), "{source}");
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
/// [`REPO_WRITES`] name on a commission repo.
fn field_writes(file_name: &str, file: &syn::File) -> Vec<(String, String, String)> {
    named_writes(file_name, file, &FIELD_WRITES, &REPO_WRITES)
}

/// The commission port's traits a path may name a write through.
const REPO_TRAITS: [&str; 2] = [COMMISSION_WRITES, COMMISSION_REPO];

/// Each write in `file` of one of `by_name`, named anywhere, and of one of `on_repo` (names too
/// common to count elsewhere), on a `commissions()` receiver or a local bound from one, or by a
/// path through the commission port's traits.
fn named_writes(
    file_name: &str,
    file: &syn::File,
    by_name: &[&str],
    on_repo: &[&str],
) -> Vec<(String, String, String)> {
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
                let by_name = by_name.contains(&ident.as_str()) && !defined_at(level, at);
                let on_a_receiver = at >= 2
                    && punct_at(level, at - 1, '.')
                    && receiver_is_the_repo(level, at - 2, &bound);
                let through_a_trait = at >= 3
                    && path_separator_at(level, at - 2)
                    && ident_at(level, at - 3)
                        .is_some_and(|segment| REPO_TRAITS.contains(&segment.as_str()));
                let on_the_repo =
                    on_repo.contains(&ident.as_str()) && (on_a_receiver || through_a_trait);
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
        "impl Archive<'_> { async fn stamp(&self, uow: Unit<'_>) { uow.commissions().set_archived(&id, Some(at)).await?; } }",
        "async fn stamp(uow: Unit<'_>) { let mut commissions = uow.commissions(); commissions.set_archived(&id, None).await?; }",
        "async fn copy(uow: Unit<'_>) { uow.commissions().create(&commission).await?; }",
        "async fn copy(uow: Unit<'_>) { let mut repo = uow.commissions(); repo.create(&commission).await?; }",
        "async fn purge(uow: Unit<'_>) { UnitOfWork::commissions(uow).delete(&id).await?; }",
        "async fn purge(uow: Unit<'_>) { let repo = uow.commissions(); (*repo).delete(&id).await?; }",
        "fn setter() { let set = CommissionRepo::set_archived; }",
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
        "trait Repo { async fn set_archived(&mut self, id: &CommissionId, at: Option<DateTimeUtc>); }",
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
const WRITE_VIEW_READS: [&str; 1] = ["commission_has_facts"];

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

/// The call sites of the writes clippy bans, in `application`, by file, function and method:
/// each under its `#[expect]`, one each. A second site fails, under a copied `#[expect]` too;
/// a site leaves when its slice routes it through the den's write gate.
const FROZEN_BANNED_WRITES: [(&str, &str, &str); 6] = [
    ("commission/delete.rs", "Commissions::delete", "delete"),
    (
        "commission/invitations/issue.rs",
        "Invitations::issue",
        "create_seat_invitation",
    ),
    (
        "commission/invitations/revoke.rs",
        "Invitations::revoke",
        "revoke_seat_invitation",
    ),
    (
        "commission/seats/declare.rs",
        "Seats::declare",
        "declare_seat",
    ),
    ("commission/view/grant.rs", "View::grant", "grant_view"),
    ("commission/view/revoke.rs", "View::revoke", "revoke_view"),
];

/// Each call of a write clippy bans in `file`: a [`BANNED_WRITES`] name anywhere, and one of
/// [`REPO_WRITES`] among them on a commission repo.
fn banned_writes(file_name: &str, file: &syn::File) -> Vec<(String, String, String)> {
    let (on_repo, by_name): (Vec<&str>, Vec<&str>) = BANNED_WRITES
        .iter()
        .copied()
        .partition(|name| REPO_WRITES.contains(name));
    named_writes(file_name, file, &by_name, &on_repo)
}

/// Each call of a write clippy bans in the production sources under `src`, sorted.
fn banned_writes_under(src: &Path) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = walk(src)
        .production
        .iter()
        .flat_map(|path| banned_writes(&relative(path, src), &parse(path)))
        .collect();
    out.sort();
    out
}

#[test]
fn the_banned_writes_are_called_only_at_their_frozen_sites() {
    let found = banned_writes_under(&application_src());

    let frozen: Vec<(String, String, String)> = FROZEN_BANNED_WRITES
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
    let named: BTreeSet<&str> = FROZEN_BANNED_WRITES
        .iter()
        .map(|(_, _, method)| *method)
        .collect();
    let banned: BTreeSet<&str> = BANNED_WRITES.into_iter().collect();
    assert_eq!(named, banned, "each banned write has its one frozen site");
    let grant_file = "commission/view/grant.rs";
    let grant_source =
        std::fs::read_to_string(application_src().join(grant_file)).expect("grant.rs reads");
    let expect_start = grant_source
        .find("        #[expect(")
        .expect("grant.rs holds its #[expect]");
    let call_end = grant_source[expect_start..]
        .find(".await?;")
        .map(|offset| expect_start + offset + ".await?;".len())
        .expect("the expected call ends");
    let site = &grant_source[expect_start..call_end];
    assert!(site.contains("grant_view"), "{site}");
    let copied = grant_source.replacen(site, &format!("{site}\n{site}"), 1);
    let copied_file = syn::parse_file(&copied).expect("the copied site parses");
    let twice = banned_writes(grant_file, &copied_file);
    assert_eq!(twice.len(), 2, "{twice:?}");
    let new_sites = [
        "impl Seats<'_> { async fn again(&self, uow: Unit<'_>) { uow.commissions().declare_seat(&seat).await?; } }",
        "fn granter() { let grant = CommissionWrites::grant_view; }",
        "async fn purge(uow: Unit<'_>) { let repo = uow.commissions(); CommissionRepo::delete(&mut *repo, &id).await?; }",
        "async fn purge(uow: Unit<'_>) { CommissionWrites::delete(uow.commissions(), &id).await?; }",
        "use domain::ports::CommissionWrites::revoke_view as unview;",
        "async fn issue(uow: Unit<'_>) { tokio::join!(async { uow.commissions().create_seat_invitation(&invitation).await }); }",
    ];
    for source in new_sites {
        let file = syn::parse_file(source).expect("the planted use case parses");

        let writes = banned_writes("commission/extend.rs", &file);

        assert_eq!(writes.len(), 1, "{source}: {writes:#?}");
    }
    let not_banned = [
        "async fn upload(ports: &Ports, key: FileKey) { ports.files.delete(key).await?; }",
        "async fn remove(uow: Unit<'_>) { uow.workflows().delete(&workflow_id).await?; }",
        "trait Repo { async fn grant_view(&mut self, id: &CommissionId); }",
    ];
    for source in not_banned {
        let file = syn::parse_file(source).expect("the planted source parses");

        let writes = banned_writes("commission/other.rs", &file);

        assert_eq!(writes, Vec::<(String, String, String)>::new(), "{source}");
    }
}

/// The write view of a commission unit.
const COMMISSION_WRITES: &str = "CommissionWrites";

/// The read view of a commission unit.
const COMMISSION_READS: &str = "CommissionReads";

/// What a unit's `commissions()` vends: the read view and the write view, nothing else.
const COMMISSION_REPO: &str = "CommissionRepo";

/// The read view's methods: lookups and row locks, never a write.
const UNIT_READS: [&str; 7] = [
    "find",
    "find_for_update",
    "standing_for_update",
    "lock_for_system",
    "late_owed",
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
        BTreeSet::from(
            [
                "clear_dispute",
                "create",
                "set_deadline",
                "set_deadline_status",
                "set_direction_status",
                "set_dispute",
                "set_lifecycle",
                "set_maturity",
            ]
            .map(str::to_owned)
        ),
        "create takes the gate's create capability, the lifecycle, dispute, direction, deadline \
         and Delayed writes a Participant's, and the rating the Owner's, each in place of the id"
    );
    let (held, homes) = held_names(&file);
    assert_eq!(homes, Vec::<String>::new());
    for name in [
        WRITE_CAPABILITY,
        NEW,
        CREATOR,
        COMMISSION_ID,
        AS_OWNER,
        AS_PARTICIPANT,
    ] {
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

/// The role of the system's own act: bound to no port method in 0.2, so its capability
/// writes nothing.
const AS_SYSTEM: &str = "AsSystem";

#[test]
fn no_port_method_takes_the_system_s_capability() {
    let port = std::fs::read_to_string(commission_port_file()).expect("the port file is readable");
    let roles: Vec<&str> = BOUND_ROLES.iter().map(|(role, _)| *role).collect();

    let named = identifiers(port.parse::<TokenStream>().expect("the port file lexes"));

    assert!(!roles.contains(&AS_SYSTEM), "{roles:?}");
    assert!(
        !named.contains(&AS_SYSTEM.to_owned()),
        "the port file names {AS_SYSTEM}"
    );
    let planted = "use crate::elements::commission::{AsSystem, CommissionId};\nuse kernel::ports::WriteCapability;\ntrait CommissionWrites: Send { async fn mark_late(&mut self, capability: &WriteCapability<CommissionId, AsSystem>) -> anyhow::Result<bool>; }";
    let file = syn::parse_file(planted).expect("the planted trait parses");
    let findings = port_findings(&file, &BTreeSet::new());
    assert_eq!(
        findings,
        ["CommissionWrites::mark_late takes no write capability and is on no list"]
    );
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
    let owner_planted = source.replacen(
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
    let last_write = "        change: &DelayedChange,\n    ) -> anyhow::Result<bool>;\n";
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

// A capped write's other parameters: only the named values, none holding a commission's id.

/// The values a capped write may take beside its capability, each by its bare name behind at
/// most one shared reference: a create's birth and start, and each act's checked change.
const CAPPED_VALUES: [&str; 9] = [
    "CommissionBirth",
    "NewCommission",
    "LifecycleMove",
    "Dispute",
    "DisputeClear",
    "DirectionChange",
    "DeadlineChange",
    "DelayedChange",
    "MaturitySet",
];

/// The module the port file imports each of [`CAPPED_VALUES`] from.
const CAPPED_VALUE_HOME: &str = "crate::elements::commission";

/// The bare name `ty` is behind at most one shared reference.
fn value_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Reference(reference) if reference.mutability.is_none() => bare_name(&reference.elem),
        other => bare_name(other),
    }
}

/// The values `file` holds to their home, each imported once from [`CAPPED_VALUE_HOME`] and
/// bound by nothing else; and a finding for every other binding or import of one.
fn held_values(file: &syn::File) -> (BTreeSet<String>, Vec<String>) {
    let names: BTreeSet<String> = CAPPED_VALUES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let bound = bindings(file.to_token_stream(), &names);
    let imported = imports(file);
    let mut held = BTreeSet::new();
    let mut findings: Vec<String> = bound
        .iter()
        .map(|(binder, name)| format!("the port file binds `{name}` by {binder}"))
        .collect();
    for name in CAPPED_VALUES {
        let from: Vec<&str> = imported
            .iter()
            .filter(|(imported_name, _)| imported_name == name)
            .map(|(_, path)| path.as_str())
            .collect();
        let unbound = bound.iter().all(|(_, bound_name)| bound_name != name);
        if from == [CAPPED_VALUE_HOME] && unbound {
            held.insert(name.to_owned());
        } else if !from.is_empty() {
            findings.push(format!(
                "the port file imports `{name}` from {from:?}, not once from `{CAPPED_VALUE_HOME}`"
            ));
        }
    }
    (held, findings)
}

/// What breaks the value rule in the port file `file`: a binding or a stray import of a
/// value, and each parameter of a capped write, its capability aside, that is not a held
/// value. Answers the values the capped writes take, too.
fn capped_value_findings(file: &syn::File) -> (BTreeSet<String>, Vec<String>) {
    let (held, mut findings) = held_values(file);
    let mut taken = BTreeSet::new();
    let (capability_names, _) = held_names(file);
    let ids = commission_id_names(file);
    for tr in traits_named(file, COMMISSION_WRITES) {
        for item in &tr.items {
            let TraitItem::Fn(method) = item else {
                continue;
            };
            if !is_capped(&method.sig, &capability_names, &ids) {
                continue;
            }
            let typed = method.sig.inputs.iter().filter_map(|input| match input {
                syn::FnArg::Typed(typed) => Some(typed.ty.as_ref()),
                syn::FnArg::Receiver(_) => None,
            });
            for ty in typed {
                let names_the_capability = identifiers(ty.to_token_stream())
                    .iter()
                    .any(|ident| ident == WRITE_CAPABILITY);
                if names_the_capability {
                    continue;
                }
                match value_name(ty).filter(|name| held.contains(name)) {
                    Some(name) => {
                        taken.insert(name);
                    }
                    None => findings.push(format!(
                        "{COMMISSION_WRITES}::{} takes `{}`, not one of the named values",
                        unraw(&method.sig.ident),
                        ty.to_token_stream()
                    )),
                }
            }
        }
    }
    (taken, findings)
}

/// Each type `files` (workflow's sources) define, at any item depth, by name: the tokens of
/// its fields, or of the type an alias names.
fn defined_types(files: &[(String, String)]) -> BTreeMap<String, Vec<TokenStream>> {
    let mut out: BTreeMap<String, Vec<TokenStream>> = BTreeMap::new();
    for (_, source) in files {
        let parsed = syn::parse_file(source).expect("the source parses");
        for item in every_item(&parsed.items) {
            let (name, tokens) = match item {
                Item::Struct(structure) => (&structure.ident, structure.fields.to_token_stream()),
                Item::Enum(enumeration) => {
                    let variants: TokenStream = enumeration
                        .variants
                        .iter()
                        .map(|variant| variant.fields.to_token_stream())
                        .collect();
                    (&enumeration.ident, variants)
                }
                Item::Union(union) => (&union.ident, union.fields.to_token_stream()),
                Item::Type(alias) => (&alias.ident, alias.ty.to_token_stream()),
                _ => continue,
            };
            out.entry(unraw(name)).or_default().push(tokens);
        }
    }
    out
}

/// What breaks the rule that no named value holds a commission's id, in workflow's sources
/// `files`: a value defined other than once, and each value whose fields name one of `ids`,
/// directly or through any type defined there.
fn value_field_findings(files: &[(String, String)], ids: &BTreeSet<String>) -> Vec<String> {
    let defined = defined_types(files);
    let mut out = Vec::new();
    for value in CAPPED_VALUES {
        let count = defined.get(value).map_or(0, Vec::len);
        if count != 1 {
            out.push(format!(
                "`{value}` is defined {count} times in workflow, not once"
            ));
        }
        let mut seen: BTreeSet<String> = BTreeSet::from([value.to_owned()]);
        let mut path: Vec<String> = vec![value.to_owned()];
        holds_an_id(value, &defined, ids, &mut seen, &mut path, &mut out);
    }
    out
}

fn holds_an_id(
    name: &str,
    defined: &BTreeMap<String, Vec<TokenStream>>,
    ids: &BTreeSet<String>,
    seen: &mut BTreeSet<String>,
    path: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    let Some(definitions) = defined.get(name) else {
        return;
    };
    for tokens in definitions {
        for ident in identifiers(tokens.clone()) {
            if ids.contains(&ident) {
                out.push(format!(
                    "`{}` holds `{ident}` through {}",
                    path[0],
                    path.join(" → ")
                ));
            } else if defined.contains_key(&ident) && seen.insert(ident.clone()) {
                path.push(ident.clone());
                holds_an_id(&ident, defined, ids, seen, path, out);
                path.pop();
            }
        }
    }
}

/// The workflow crate's production sources, each by its path relative to its `src`.
fn workflow_sources() -> Vec<(String, String)> {
    sources_under(&crates().join("workflow/src"), &[])
}

/// The names that stand for a commission's id in `files`: [`COMMISSION_ID`] and every alias of
/// it they make.
fn ids_in(files: &[(String, String)]) -> BTreeSet<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    with_aliases(&[COMMISSION_ID], &sources)
}

#[test]
fn capped_parameters_come_from_the_value_list() {
    let port_source =
        std::fs::read_to_string(commission_port_file()).expect("the port file is readable");
    let port = syn::parse_file(&port_source).expect("the port file parses");
    let workflow = workflow_sources();

    let (taken, findings) = capped_value_findings(&port);
    let fields = value_field_findings(&workflow, &ids_in(&workflow));

    assert_eq!(findings, Vec::<String>::new());
    assert_eq!(fields, Vec::<String>::new());
    let every_value: BTreeSet<String> = CAPPED_VALUES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        taken, every_value,
        "the capped writes take each named value"
    );
    let deadline_param = "        change: &DeadlineChange,\n";
    assert!(port_source.contains(deadline_param));
    for planted_type in [
        "&uuid::Uuid",
        "&NodeId",
        "&CommissionSummary",
        "&str",
        "DeadlineChange<u8>",
        "&mut DeadlineChange",
        "&&DeadlineChange",
    ] {
        let planted = port_source.replacen(
            deadline_param,
            &format!("        change: {planted_type},\n"),
            1,
        );
        let file = syn::parse_file(&planted).expect("the planted port parses");

        let (_, planted_findings) = capped_value_findings(&file);

        assert!(!planted_findings.is_empty(), "{planted_type}");
    }
    let second = port_source.replacen(
        deadline_param,
        "        change: &DeadlineChange,\n        also: &CommissionBirth,\n        node: &NodeId,\n",
        1,
    );
    let (_, second_findings) = capped_value_findings(&syn::parse_file(&second).expect("it parses"));
    assert_eq!(
        second_findings,
        ["CommissionWrites::set_deadline takes `& NodeId`, not one of the named values"]
    );
    let rebound = [
        format!("{port_source}\ntype DeadlineChange = CommissionSummary;\n"),
        format!(
            "{port_source}\nuse crate::elements::commission::CommissionSummary as DeadlineChange;\n"
        ),
        port_source.replacen(
            "use crate::elements::",
            "use crate::elements::commission::deadline::DeadlineChange;\nuse crate::elements::",
            1,
        ),
    ];
    for planted in rebound {
        let file = syn::parse_file(&planted).expect("the planted port parses");

        let (_, planted_findings) = capped_value_findings(&file);

        assert!(!planted_findings.is_empty(), "{planted}");
    }
    let deadline_file = "elements/commission/deadline.rs";
    let change_fields = "pub struct DeadlineChange {\n    from: Option<DateTimeUtc>,\n";
    let (_, deadline_source) = workflow
        .iter()
        .find(|(file, _)| file == deadline_file)
        .expect("workflow's deadline file");
    assert!(deadline_source.contains(change_fields));
    let gains = [
        "    commission: CommissionId,\n",
        "    summary: Option<CommissionSummary>,\n",
        "    held: Vec<(DateTimeUtc, Box<CommissionId>)>,\n",
        "    held: HeldFor,\n",
    ];
    for gained in gains {
        let planted_source =
            deadline_source.replacen(change_fields, &format!("{change_fields}{gained}"), 1);
        let planted_source =
            format!("{planted_source}\npub type HeldFor = Option<CommissionId>;\n");
        let planted = with_file(&workflow, deadline_file, &planted_source);

        let planted_findings = value_field_findings(&planted, &ids_in(&planted));

        assert!(
            planted_findings
                .iter()
                .any(|finding| finding.starts_with("`DeadlineChange` holds")),
            "{gained}: {planted_findings:?}"
        );
    }
    let aliased = format!(
        "{}\nuse crate::elements::commission::CommissionId as Held;\n",
        deadline_source.replacen(
            change_fields,
            &format!("{change_fields}    held: Held,\n"),
            1
        )
    );
    let planted = with_file(&workflow, deadline_file, &aliased);
    assert!(!value_field_findings(&planted, &ids_in(&planted)).is_empty());
    let twice = with_file(
        &workflow,
        "elements/commission/twin.rs",
        "pub struct DeadlineChange;",
    );
    assert!(!value_field_findings(&twice, &ids_in(&twice)).is_empty());
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

/// The test seeder that writes a commission's lifecycle step: no production source of the
/// adapter calls it, and no driver names it.
const LIFECYCLE_SEEDER: &str = "seed_lifecycle_step";

/// Its one naming in the adapter: its definition.
const LIFECYCLE_SEEDER_DEFINITION: (&str, &str, usize) =
    ("MemBackend::seed_lifecycle_step", LIFECYCLE_SEEDER, 1);

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

/// The elements of a `use` group's contents `stream`, nested groups unwrapped at any depth:
/// `a, {b, {c::d}}` gives `a`, `b` and `c::d`.
fn group_elements(stream: TokenStream) -> Vec<Vec<TokenTree>> {
    comma_separated(stream)
        .into_iter()
        .flat_map(|element| match element.as_slice() {
            [TokenTree::Group(group)] if group.delimiter() == Delimiter::Brace => {
                group_elements(group.stream())
            }
            _ => vec![element],
        })
        .collect()
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
/// a `queries::commission` path, `commission` in a group under `queries::` (nested groups at
/// any depth), a glob from `queries`, or an alias of `queries`.
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
                let reached = group_elements(group.stream())
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
/// (`commission as sql`, or `commission::{self as sql}` in a group at any depth), and each
/// rename of those.
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
                    for element in group_elements(group.stream()) {
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
/// `m::{f, …}` (nested groups at any depth), or a glob `m::*`.
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
                let named = group_elements(group.stream())
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

/// The brace contents of each struct literal or pattern of one of `names` (or `Self`, when
/// `self_named`) in `level`, as [`struct_literal_at`] finds them.
fn struct_literals(
    level: &[TokenTree],
    names: &BTreeSet<String>,
    self_named: bool,
) -> Vec<TokenStream> {
    (0..level.len())
        .filter_map(|at| struct_literal_at(level, at, names, self_named))
        .map(|(_, contents)| contents)
        .collect()
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

/// The write view's write helpers in the commission file among `files`: its inherent methods
/// that name a write function, directly or through another of them.
fn pg_commission_file_helpers(
    files: &[(String, String)],
    write_fns: &BTreeSet<String>,
) -> BTreeSet<String> {
    let Some((_, source)) = files.iter().find(|(file, _)| file == COMMISSION_FILE) else {
        return BTreeSet::new();
    };
    let parsed = syn::parse_file(source).expect("the commission file parses");
    let file_pieces = pieces(&parsed);
    let file_levels: Vec<Vec<TokenTree>> = file_pieces
        .iter()
        .flat_map(|piece| levels(piece.tokens.clone()))
        .collect();
    let modules = query_module_names(&file_levels);
    pg_write_helpers(&file_pieces, &modules, write_fns)
}

/// Every way sqlx 0.9 takes SQL text, by name: the query functions and macros, `raw_sql`, the
/// query builder, the text wrappers and their conversion, the executor's methods (each takes a
/// `&'static str`, a leaked one included), a transaction opened with a statement, the raw
/// `COPY`, and a migrator built from a folder. Each lets adapter-pg send text the pin can't
/// read, so only the generated queries and [`SQL_TEXT_EXEMPTIONS`] name one.
const SQL_TEXT_DOORS: [&str; 46] = [
    "query",
    "query_with",
    "query_as",
    "query_as_with",
    "query_scalar",
    "query_scalar_with",
    "__query_with_result",
    "__query_scalar_with_result",
    "query_statement",
    "query_statement_with",
    "query_statement_as",
    "query_statement_as_with",
    "query_statement_scalar",
    "query_statement_scalar_with",
    "query_unchecked",
    "query_as_unchecked",
    "query_scalar_unchecked",
    "query_file",
    "query_file_as",
    "query_file_scalar",
    "query_file_unchecked",
    "query_file_as_unchecked",
    "query_file_scalar_unchecked",
    "raw_sql",
    "RawSql",
    "QueryBuilder",
    "AssertSqlSafe",
    "SqlStr",
    "SqlSafeStr",
    "into_sql_str",
    "Execute",
    "execute",
    "execute_many",
    "fetch",
    "fetch_many",
    "fetch_all",
    "fetch_one",
    "fetch_optional",
    "prepare",
    "prepare_with",
    "describe",
    "begin_with",
    "try_begin_with",
    "copy_in_raw",
    "copy_out_raw",
    "Migrator",
];

/// sqlx's migration macro, which embeds a folder of SQL: a door only when called, since its
/// module (`sqlx::migrate::…`) carries the same name.
const MIGRATE_MACRO: &str = "migrate";

/// The context [`sql_text_door_findings`] gives a `use` item at the top of a file.
const TOP_LEVEL_IMPORTS: &str = "the file's imports";

/// The SQL-text doors adapter-pg's hand-written sources may name, by file, context and count:
/// the crate root's imports, `migrator`, which turns the embedded migration files into
/// migrations, and `schema_status`'s one ledger probe.
const SQL_TEXT_EXEMPTIONS: [(&str, &str, &str, usize); 6] = [
    ("lib.rs", TOP_LEVEL_IMPORTS, "SqlSafeStr", 1),
    ("lib.rs", TOP_LEVEL_IMPORTS, "Migrator", 1),
    ("lib.rs", "migrator", "into_sql_str", 1),
    ("lib.rs", "migrator", "Migrator", 3),
    ("lib.rs", "schema_status", "query_scalar", 1),
    ("lib.rs", "schema_status", "fetch_one", 1),
];

/// Each naming of a SQL-text door in adapter-pg's sources `files` (each by path relative to its
/// `src`, the generated queries aside), past the allowance `exemptions` holds: one of
/// [`SQL_TEXT_DOORS`], a call of [`MIGRATE_MACRO`], or a name the sources bind to either (a
/// rename or a type alias, at any depth).
fn sql_text_door_namings(
    files: &[(String, String)],
    exemptions: &[(&str, &str, &str, usize)],
) -> Vec<Naming> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let mut names = token_aliases(&SQL_TEXT_DOORS, &sources);
    let macro_aliases: Vec<String> = token_aliases(&[MIGRATE_MACRO], &sources)
        .into_iter()
        .filter(|name| name != MIGRATE_MACRO)
        .collect();
    names.extend(macro_aliases);
    let mut out = Vec::new();
    for (file, source) in files {
        let shown = format!("adapter-pg/src/{file}");
        let parsed =
            syn::parse_file(source).unwrap_or_else(|error| panic!("{shown} parses: {error}"));
        let mut file_pieces = pieces(&parsed);
        for piece in &mut file_pieces {
            if is_use_item(piece) && piece.context.starts_with("use ") {
                piece.context = TOP_LEVEL_IMPORTS.to_owned();
            }
        }
        let allowed: Vec<(&str, &str, usize)> = exemptions
            .iter()
            .filter(|(exempt_file, ..)| exempt_file == file)
            .map(|(_, context, name, count)| (*context, *name, *count))
            .collect();
        out.extend(named_past(&shown, &file_pieces, &names, &allowed));
        for piece in &file_pieces {
            for level in levels(piece.tokens.clone()) {
                let calls = (0..level.len()).filter(|at| {
                    ident_at(&level, *at).as_deref() == Some(MIGRATE_MACRO)
                        && punct_at(&level, at + 1, '!')
                });
                for _ in calls {
                    let naming = (
                        shown.clone(),
                        piece.context.clone(),
                        format!("{MIGRATE_MACRO}!"),
                    );
                    out.push(naming);
                }
            }
        }
    }
    out
}

/// What sends SQL text the pin can't read in adapter-pg's sources `files`: each naming of a
/// SQL-text door past [`SQL_TEXT_EXEMPTIONS`].
fn sql_text_door_findings(files: &[(String, String)]) -> Vec<String> {
    show(&sql_text_door_namings(files, &SQL_TEXT_EXEMPTIONS))
}

/// What breaks the PostgreSQL pin in the adapter's sources `files` (each by path relative to
/// its `src`, the generated queries aside): outside the commission file, any reach into the
/// generated commission module; in it, a write function named or a write method called
/// outside the write view; in any file, a write helper called outside the write view; a
/// write-view literal outside the unit's `commissions()`; SQL in Rust that writes a
/// commission table or names a `.sql` file; and SQL text handed to sqlx past the generated
/// queries and the named exemptions ([`sql_text_door_findings`]), so every statement the
/// adapter sends is a literal the pin reads.
fn pg_pin_findings(
    files: &[(String, String)],
    write_fns: &BTreeSet<String>,
    write_methods: &BTreeSet<String>,
) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let views = token_aliases(&[PG_WRITE_VIEW], &sources);
    let helpers = pg_commission_file_helpers(files, write_fns);
    let mut out = sql_text_door_findings(files);
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
        let mut literals_left = usize::from(file == PG_UNIT_FILE);
        for piece in &file_pieces {
            let context = &piece.context;
            let self_named = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| views.contains(name));
            for level in levels(piece.tokens.clone()) {
                let in_the_view = in_commission && in_the_pg_write_view(piece);
                if !in_the_view {
                    for name in calls_into(&level, &helpers) {
                        out.push(format!("{shown}: write helper `{name}` in {context}"));
                    }
                }
                if !in_commission {
                    for reach in query_module_reaches(&level) {
                        out.push(format!("{shown}: {reach} in {context}"));
                    }
                } else if !in_the_view {
                    for name in write_fn_namings(&level, &modules, write_fns) {
                        out.push(format!("{shown}: write query `{name}` in {context}"));
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
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::queries::{{commission}}; Ok(commission::set_archived(&mut *self.conn, uuid::Uuid::from(*commission), None).await? == 1)",
            ),
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use crate::{queries::{column, {self as q}}}; Ok(q::commission::delete(&mut *self.conn, uuid::Uuid::from(*commission)).await? == 1)",
            ),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use sql::{find, {set_archived as archive_row}};".to_owned(),
        ),
        (
            COMMISSION_FILE,
            "use crate::{PgColumnStore, queries::commission as sql};",
            "use crate::queries::commission::{{self as rows}};\nfn wipe(pool: &PgPool) { let _ = rows::revoke_view; }".to_owned(),
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn retire_card(&mut self, view: &mut crate::PgCommissionWrites<'_>, element: &NewElement) -> anyhow::Result<()> { view.insert_element(element).await }".to_owned(),
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn probe(&mut self) -> anyhow::Result<()> { sqlx::query(\"SELECT id FROM commission WHERE id = $1\").execute(&mut *self.conn).await?; Ok(()) }".to_owned(),
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
    ];
    for (file, anchor, addition) in kept {
        let planted = planted_after(&files, file, anchor, addition);

        let found = pg_pin_findings(&planted, &write_fns, &write_methods);

        assert_eq!(found, Vec::<String>::new(), "{file}: {addition}");
    }
}

#[test]
fn the_pg_pin_names_each_sql_text_door_outside_its_exemptions() {
    let files = pg_sources();

    let unexempted = sql_text_door_namings(&files, &[]);

    let mut counted: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    for (file, context, name) in unexempted {
        *counted.entry((file, context, name)).or_default() += 1;
    }
    let exempted: BTreeMap<(String, String, String), usize> = SQL_TEXT_EXEMPTIONS
        .iter()
        .map(|(file, context, name, count)| {
            let key = (
                format!("adapter-pg/src/{file}"),
                (*context).to_owned(),
                (*name).to_owned(),
            );
            (key, *count)
        })
        .collect();
    assert_eq!(
        counted, exempted,
        "the exemptions are today's namings, exactly"
    );
    assert_eq!(sql_text_door_findings(&files), Vec::<String>::new());
    let markup =
        "    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {";
    let column_writes = "impl ColumnWrites for PgColumnWrites<'_> {";
    let imports = "pub use sqlx::PgPool;";
    let ledger_probe = "    let migrator = migrator();";
    let retire = |body: &str| {
        format!(
            "    async fn retire_card(&mut self, commission: &CommissionId) -> anyhow::Result<bool> {{ {body} }}"
        )
    };
    let herons_plant = "        let table = \"commission\";\n        let assembled = format!(\"UPDATE {table} SET archived_at = NULL WHERE id = $1\");\n        sqlx::query(sqlx::AssertSqlSafe(assembled))\n            .bind(uuid::Uuid::from(markup.commission_id))\n            .execute(&mut *self.conn)\n            .await?;";
    let refused = [
        (
            COMMISSION_FILE,
            markup,
            herons_plant.to_owned(),
            "adapter-pg/src/commission.rs: `AssertSqlSafe` in PgCommissionWrites::add_markup",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use sqlx::AssertSqlSafe as Trusted; let text = Trusted(format!(\"UPDATE {} SET archived_at = NULL\", \"commission\")); Ok(sqlx::Executor::execute(&mut *self.conn, text).await?.rows_affected() == 1)",
            ),
            "adapter-pg/src/workflow.rs: `Trusted` in PgColumnWrites::retire_card",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let written = sqlx::raw_sql(\"SELECT 1\").execute(&mut *self.conn).await?; Ok(written.rows_affected() == 1)",
            ),
            "adapter-pg/src/workflow.rs: `raw_sql` in PgColumnWrites::retire_card",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let mut built = sqlx::QueryBuilder::<sqlx::Postgres>::new(\"UPDATE \"); built.push(\"commission\").push(\" SET archived_at = NULL\"); Ok(built.build().execute(&mut *self.conn).await?.rows_affected() == 1)",
            ),
            "adapter-pg/src/workflow.rs: `QueryBuilder` in PgColumnWrites::retire_card",
        ),
        (
            "lib.rs",
            imports,
            "pub async fn ledger_rows(pool: &PgPool) -> Result<i64, sqlx::Error> { sqlx::query_scalar(\"SELECT count(*) FROM _sqlx_migrations\").fetch_one(pool).await }".to_owned(),
            "adapter-pg/src/lib.rs: `query_scalar` in ledger_rows",
        ),
        (
            "lib.rs",
            ledger_probe,
            "    let _: bool = sqlx::query_scalar(\"SELECT true\").fetch_one(pool).await?;".to_owned(),
            "adapter-pg/src/lib.rs: `query_scalar` in schema_status",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let text: &'static str = Box::leak(format!(\"UPDATE {} SET archived_at = NULL\", \"commission\").into_boxed_str()); Ok(sqlx::Executor::execute(&mut *self.conn, text).await?.rows_affected() == 1)",
            ),
            "adapter-pg/src/workflow.rs: `execute` in PgColumnWrites::retire_card",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let text = sqlx::SqlStr::from_static(\"SELECT 1\"); Ok(sqlx::Executor::execute(&mut *self.conn, text).await?.rows_affected() == 1)",
            ),
            "adapter-pg/src/workflow.rs: `SqlStr` in PgColumnWrites::retire_card",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "use sqlx::SqlSafeStr; let text = \"SELECT 1\".into_sql_str(); Ok(self.conn.begin_with(text).await.is_ok())",
            ),
            "adapter-pg/src/workflow.rs: `into_sql_str` in PgColumnWrites::retire_card",
        ),
        (
            "workflow.rs",
            column_writes,
            retire(
                "let text = String::from(\"COPY commission FROM STDIN\"); Ok(self.conn.copy_in_raw(&text).await.is_ok())",
            ),
            "adapter-pg/src/workflow.rs: `copy_in_raw` in PgColumnWrites::retire_card",
        ),
        (
            "lib.rs",
            imports,
            "pub async fn migrate_elsewhere(pool: &PgPool) -> anyhow::Result<()> { sqlx::migrate!(\"./elsewhere\").run(pool).await?; Ok(()) }".to_owned(),
            "adapter-pg/src/lib.rs: `migrate!` in migrate_elsewhere",
        ),
        (
            "lib.rs",
            imports,
            "pub async fn migrate_from(pool: &PgPool, folder: &std::path::Path) -> anyhow::Result<()> { sqlx::migrate::Migrator::new(folder).await?.run(pool).await?; Ok(()) }".to_owned(),
            "adapter-pg/src/lib.rs: `Migrator` in migrate_from",
        ),
        (
            "lib.rs",
            imports,
            "type Assembled = sqlx::AssertSqlSafe<String>;".to_owned(),
            "adapter-pg/src/lib.rs: `AssertSqlSafe` in type Assembled",
        ),
    ];
    for (file, anchor, addition, expected) in &refused {
        let planted = planted_after(&files, file, anchor, addition);

        let found = sql_text_door_findings(&planted);

        assert!(
            found.iter().any(|finding| finding == expected),
            "{addition}: {found:?}"
        );
    }
    let aliased_elsewhere = planted_after(
        &files,
        "lib.rs",
        imports,
        "pub use sqlx::AssertSqlSafe as Trusted;",
    );
    let used_elsewhere = planted_after(
        &aliased_elsewhere,
        "workflow.rs",
        column_writes,
        &retire("let _ = crate::Trusted(String::new()); Ok(true)"),
    );
    let found = sql_text_door_findings(&used_elsewhere);
    assert!(
        found.contains(
            &"adapter-pg/src/workflow.rs: `Trusted` in PgColumnWrites::retire_card".to_owned()
        ),
        "{found:?}"
    );
    let kept = [
        (
            "lib.rs",
            imports,
            "pub fn refusal(error: sqlx::migrate::MigrateError) -> String { error.to_string() }",
        ),
        (
            "workflow.rs",
            column_writes,
            "    async fn probe(&mut self) -> anyhow::Result<()> { column_sql::clear_cards(&mut *self.conn, uuid::Uuid::nil()).await?; Ok(()) }",
        ),
    ];
    for (file, anchor, addition) in kept {
        let planted = planted_after(&files, file, anchor, addition);

        let found = sql_text_door_findings(&planted);

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

/// Whether `level` holds an `=` from `from` up to `to` outside angle brackets: an assignment,
/// where a type's `Item = T` binding sits inside them.
fn assigns_between(level: &[TokenTree], from: usize, to: usize) -> bool {
    let mut depth = 0usize;
    (from..to).any(|at| {
        if punct_at(level, at, '<') {
            depth += 1;
        } else if closes_angle_at(level, at) {
            depth = depth.saturating_sub(1);
        }
        depth == 0 && punct_at(level, at, '=')
    })
}

/// The local a `let` binds to the expression from `start` to `end`: `let [mut] x [: T] = …;`,
/// with no assignment between the name and that expression.
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
    let typed_alone =
        punct_at(level, name_at + 1, ':') && !assigns_between(level, name_at + 1, equals);
    let plain = name_at + 1 == equals || typed_alone;
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
/// pattern naming the map outside staging; the write view named outside its definition, its
/// impls, a `use` and the unit's `commissions()`, which names it once; and the lifecycle
/// seeder named anywhere but its definition. Aliases included.
fn mem_pin_findings(files: &[(String, String)]) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let rows = token_aliases(&[STORED_COMMISSION], &sources);
    let backends = token_aliases(&[MEM_BACKEND], &sources);
    let views = token_aliases(&[MEM_WRITE_VIEW], &sources);
    let seeders = token_aliases(&[LIFECYCLE_SEEDER], &sources);
    let mut card_lists: BTreeMap<(String, String), usize> = COLUMN_CARD_LISTS
        .iter()
        .map(|(file, context, count)| (((*file).to_owned(), (*context).to_owned()), *count))
        .collect();
    let mut out = Vec::new();
    for (file, source) in files {
        let shown = format!("adapter-mem/src/{file}");
        let parsed =
            syn::parse_file(source).unwrap_or_else(|error| panic!("{shown} parses: {error}"));
        let file_pieces = pieces(&parsed);
        let seeder_allowance = if file == COMMISSION_FILE {
            vec![LIFECYCLE_SEEDER_DEFINITION]
        } else {
            Vec::new()
        };
        let seeder_namings = named_past(&shown, &file_pieces, &seeders, &seeder_allowance);
        out.extend(show(&seeder_namings));
        let mut view_left = usize::from(file == MEM_LIB_FILE);
        for piece in file_pieces {
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
        (
            "workflow.rs",
            column_writes,
            retire(format!(
                "let held; let _unit: () = held = &self.0.commissions; held{lock}.clear(); Ok(true)"
            )),
        ),
        (
            COMMISSION_FILE,
            store,
            "    async fn retire_card(&self, id: &CommissionId) -> anyhow::Result<bool> { self.0.seed_lifecycle_step(*id, LifecycleStep::Active); Ok(true) }".to_owned(),
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

// No later migration defines a function or trigger that writes a commission table.

/// The version stamp of the migration that drops the commission's title and visibility:
/// every migration stamped after it is read, whether that migration is present or not.
const LAST_UNREAD_MIGRATION: u64 = 20261022165210;

/// The later migrations the migration rule reads past, by file name: each may define a
/// function, trigger or view that writes a commission table.
const COMMISSION_WRITING_MIGRATIONS: [&str; 0] = [];

/// The objects a migration may define that write later, outside any use case.
const RUNNING_OBJECTS: [&str; 4] = ["FUNCTION", "PROCEDURE", "TRIGGER", "RULE"];

/// The words a PL/pgSQL statement starts after.
const STATEMENT_STARTS: [&str; 5] = ["BEGIN", "THEN", "ELSE", "LOOP", "DECLARE"];

/// The index past the end of `characters` from `from` on that `closer` ends, or their end.
fn past(characters: &[char], from: usize, closer: &[char]) -> usize {
    (from..characters.len())
        .find(|at| characters[*at..].starts_with(closer))
        .map_or(characters.len(), |at| at + closer.len())
}

/// The index past a single-quoted string opening at `at`: a doubled quote stays inside, and
/// so does an escaped one in an `E'…'` string.
fn past_the_string(characters: &[char], at: usize) -> usize {
    let escapes = at
        .checked_sub(1)
        .is_some_and(|before| matches!(characters[before], 'E' | 'e'));
    let mut index = at + 1;
    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();
        if (character == '\'' && next == Some('\'')) || (escapes && character == '\\') {
            index += 2;
        } else if character == '\'' {
            return index + 1;
        } else {
            index += 1;
        }
    }
    characters.len()
}

/// The index past the block comment opening at `at`, or the end: PostgreSQL's block comments
/// nest.
fn past_a_block_comment(characters: &[char], at: usize) -> usize {
    let mut depth = 0usize;
    let mut index = at;
    while index < characters.len() {
        let pair = (characters[index], characters.get(index + 1).copied());
        if pair == ('/', Some('*')) {
            depth += 1;
            index += 2;
        } else if pair == ('*', Some('/')) {
            depth = depth.saturating_sub(1);
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    characters.len()
}

/// The index past a quoted string, quoted identifier, dollar-quoted body or comment opening at
/// `at`, if one opens there.
fn past_a_quote(characters: &[char], at: usize) -> Option<usize> {
    let next = characters.get(at + 1).copied();
    match (characters[at], next) {
        ('\'', _) => Some(past_the_string(characters, at)),
        ('"', _) => Some(past(characters, at + 1, &['"'])),
        ('-', Some('-')) => Some(past(characters, at + 2, &['\n'])),
        ('/', Some('*')) => Some(past_a_block_comment(characters, at)),
        ('$', _) => {
            let end = (at + 1..characters.len())
                .find(|index| !(characters[*index].is_alphanumeric() || characters[*index] == '_'))
                .filter(|end| characters[*end] == '$')?;
            let tag = &characters[at..=end];
            Some(past(characters, end + 1, tag))
        }
        _ => None,
    }
}

/// `sql` cut into its statements at each `;` outside quotes, dollar quotes and comments; a
/// `BEGIN ATOMIC` body runs on to the statement that ends with `END`.
fn sql_statements(sql: &str) -> Vec<String> {
    let characters: Vec<char> = sql.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut at = 0;
    while at < characters.len() {
        if let Some(end) = past_a_quote(&characters, at) {
            current.extend(&characters[at..end]);
            at = end;
            continue;
        }
        if characters[at] == ';' {
            pieces.push(std::mem::take(&mut current));
        } else {
            current.push(characters[at]);
        }
        at += 1;
    }
    pieces.push(current);
    let mut out: Vec<String> = Vec::new();
    let mut atomic: Option<String> = None;
    for piece in pieces {
        let words = sql_words(&without_sql_comments(&piece));
        let opens = words
            .windows(2)
            .any(|pair| is_bare(pair.first(), "BEGIN") && is_bare(pair.get(1), "ATOMIC"));
        let ends = is_bare(words.last(), "END");
        match atomic.take() {
            Some(mut body) => {
                body.push(';');
                body.push_str(&piece);
                if ends {
                    out.push(body);
                } else {
                    atomic = Some(body);
                }
            }
            None if opens && !ends => atomic = Some(piece),
            None => out.push(piece),
        }
    }
    out.extend(atomic);
    out.retain(|statement| !statement.trim().is_empty());
    out
}

/// The index past the opening quote of a string that continues the one closed just before
/// `at`: PostgreSQL joins two string constants with only whitespace between them, a newline
/// among it.
fn continued_at(characters: &[char], at: usize) -> Option<usize> {
    let next = (at..characters.len()).find(|index| !characters[*index].is_whitespace())?;
    let across_lines = characters[at..next].contains(&'\n');
    (across_lines && characters[next] == '\'').then_some(next + 1)
}

/// `sql` with each single-quoted string's text read as SQL: the quotes dropped, a doubled or
/// escaped quote inside read as a space, and a string continued on a later line read on as one.
fn with_strings_opened(sql: &str) -> String {
    let characters: Vec<char> = sql.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    while at < characters.len() {
        if characters[at] != '\'' {
            out.push(characters[at]);
            at += 1;
            continue;
        }
        out.push(' ');
        at += 1;
        while at < characters.len() {
            let pair = (characters[at], characters.get(at + 1).copied());
            if pair == ('\'', Some('\'')) || pair == ('\\', Some('\'')) {
                out.push(' ');
                at += 2;
            } else if characters[at] == '\'' {
                at += 1;
                let Some(continued) = continued_at(&characters, at) else {
                    break;
                };
                at = continued;
            } else {
                out.push(characters[at]);
                at += 1;
            }
        }
        out.push(' ');
    }
    out
}

/// The index past `CREATE [OR REPLACE] [CONSTRAINT | EVENT]` at `at`, and the object it
/// defines, when it defines one of [`RUNNING_OBJECTS`].
fn running_object_at(words: &[SqlWord], at: usize) -> Option<(usize, String)> {
    if !is_bare(words.get(at), "CREATE") {
        return None;
    }
    let mut index = at + 1;
    if is_bare(words.get(index), "OR") && is_bare(words.get(index + 1), "REPLACE") {
        index += 2;
    }
    if is_bare(words.get(index), "CONSTRAINT") || is_bare(words.get(index), "EVENT") {
        index += 1;
    }
    RUNNING_OBJECTS
        .iter()
        .find(|object| is_bare(words.get(index), object))
        .map(|object| (index + 1, (*object).to_owned()))
}

/// The name of the table a word after `at` introduces: the first `keyword` past `at`, and
/// the table reference after it.
fn table_after(words: &[SqlWord], at: usize, keyword: &str) -> Option<String> {
    let found = (at..words.len()).find(|index| is_bare(words.get(*index), keyword))?;
    let start = if is_bare(words.get(found + 1), "ONLY") {
        found + 2
    } else {
        found + 1
    };
    table_name(words.get(table_name_at(words, start)))
}

/// Whether `words` assign to a trigger's new row: `NEW := …`, `NEW.c := …`, or `=` for `:=` at
/// a statement's start.
fn assigns_the_new_row(words: &[SqlWord]) -> bool {
    (0..words.len()).any(|at| {
        if !is_bare(words.get(at), "NEW") {
            return false;
        }
        let column = words.get(at + 1) == Some(&SqlWord::Other('.'));
        let operator = if column { at + 3 } else { at + 1 };
        let walrus = words.get(operator) == Some(&SqlWord::Other(':'))
            && words.get(operator + 1) == Some(&SqlWord::Other('='));
        let starts = at == 0
            || words.get(at - 1) == Some(&SqlWord::Other(';'))
            || STATEMENT_STARTS
                .iter()
                .any(|start| is_bare(words.get(at - 1), start));
        let equals = words.get(operator) == Some(&SqlWord::Other('=')) && starts;
        walrus || equals
    })
}

/// The functions `statement` defines whose body writes a commission table, by lower-case name.
fn writer_functions(statement: &str) -> Vec<String> {
    let opened = with_strings_opened(statement);
    let words = sql_words(&without_sql_comments(&opened));
    let defines: Vec<String> = (0..words.len())
        .filter_map(|at| running_object_at(&words, at))
        .filter(|(_, object)| object == "FUNCTION" || object == "PROCEDURE")
        .filter_map(|(name_at, _)| table_name(words.get(table_name_at(&words, name_at))))
        .collect();
    let writes = !commission_tables_written(&without_sql_comments(&opened)).is_empty()
        || !commission_tables_written(&opened).is_empty();
    if writes { defines } else { Vec::new() }
}

/// What one statement of a later migration does that only a listed one may: it defines a
/// function, procedure, trigger or rule and writes a commission table, assigns a trigger's new
/// row or holds a Unicode-escaped identifier; or it defines a trigger or rule on a commission
/// table, or a trigger that runs one of `writers`.
fn running_write_findings(statement: &str, writers: &BTreeSet<String>) -> Vec<String> {
    let opened = with_strings_opened(statement);
    let texts = [without_sql_comments(&opened), opened.clone()];
    let mut out = Vec::new();
    for text in &texts {
        let words = sql_words(text);
        let objects: Vec<(usize, String)> = (0..words.len())
            .filter_map(|at| running_object_at(&words, at))
            .collect();
        if objects.is_empty() {
            continue;
        }
        for table in commission_tables_written(text) {
            out.push(format!(
                "defines a function or trigger and writes `{table}`"
            ));
        }
        if assigns_the_new_row(&words) {
            out.push("assigns a trigger's new row".to_owned());
        }
        if holds_a_unicode_identifier(text) {
            out.push("holds a Unicode-escaped identifier".to_owned());
        }
        for (past, object) in &objects {
            let on = match object.as_str() {
                "TRIGGER" => table_after(&words, *past, "ON"),
                "RULE" => table_after(&words, *past, "TO"),
                _ => None,
            };
            if let Some(table) = on.filter(|table| is_a_commission_table(table)) {
                out.push(format!("defines a {object} on `{table}`"));
            }
            if object == "TRIGGER" {
                let runs = (*past..words.len())
                    .find(|at| is_bare(words.get(*at), "EXECUTE"))
                    .and_then(|at| table_name(words.get(table_name_at(&words, at + 2))));
                if let Some(function) = runs.filter(|function| writers.contains(function)) {
                    out.push(format!("defines a trigger that runs `{function}`"));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The PL/pgSQL statement that runs SQL text built at run time, whose tables no scan can read.
const DYNAMIC_SQL: &str = "EXECUTE";

/// The words after which a trigger's own `EXECUTE` names the function it runs.
const TRIGGER_RUNS: [&str; 2] = ["FUNCTION", "PROCEDURE"];

/// What a `DO` statement's body is called in a finding: it has no name.
const DO_BLOCK: &str = "a DO block";

/// `sql` without the comments outside its quotes, dollar quotes and quoted identifiers.
fn without_outer_comments(sql: &str) -> String {
    let characters: Vec<char> = sql.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    while at < characters.len() {
        let Some(end) = past_a_quote(&characters, at) else {
            out.push(characters[at]);
            at += 1;
            continue;
        };
        let pair = (characters[at], characters.get(at + 1).copied());
        if pair == ('-', Some('-')) || pair == ('/', Some('*')) {
            out.push(' ');
        } else {
            out.extend(&characters[at..end]);
        }
        at = end;
    }
    out
}

/// The texts of `statement` a body scan reads: its outer comments dropped and its single-quoted
/// strings read as SQL, once as that is and once with the comments inside the bodies dropped.
fn body_texts(statement: &str) -> [String; 2] {
    let opened = with_strings_opened(&without_outer_comments(statement));
    [without_sql_comments(&opened), opened]
}

/// What holds the body `words` (one statement) define, by lower-case name: each function or
/// procedure, or `a DO block`. Empty when the statement holds no body.
fn body_owners(words: &[SqlWord]) -> Vec<String> {
    if is_bare(words.first(), "DO") {
        return vec![DO_BLOCK.to_owned()];
    }
    (0..words.len())
        .filter_map(|at| running_object_at(words, at))
        .filter(|(_, object)| object == "FUNCTION" || object == "PROCEDURE")
        .filter_map(|(name_at, _)| table_name(words.get(table_name_at(words, name_at))))
        .collect()
}

/// Whether the `EXECUTE` at `at` is a trigger's own clause, not dynamic SQL: `EXECUTE FUNCTION`
/// or `EXECUTE PROCEDURE`, a name and its opening `(`, after a `CREATE … TRIGGER` in the same
/// statement of the body.
fn is_a_trigger_clause(words: &[SqlWord], at: usize) -> bool {
    let runs = TRIGGER_RUNS
        .iter()
        .any(|word| is_bare(words.get(at + 1), word));
    let name_end = table_name_at(words, at + 2);
    let named = matches!(
        words.get(at + 2),
        Some(SqlWord::Bare(_) | SqlWord::Quoted(_))
    ) && words.get(name_end + 1) == Some(&SqlWord::Other('('));
    let start = (0..at)
        .rev()
        .find(|index| words[*index] == SqlWord::Other(';'))
        .map_or(0, |index| index + 1);
    let in_a_trigger = (start..at).any(|index| {
        running_object_at(words, index).is_some_and(|(_, object)| object == "TRIGGER")
    });
    runs && named && in_a_trigger
}

/// Whether `words` hold an `EXECUTE` that is not a trigger's own clause.
fn runs_dynamic_sql(words: &[SqlWord]) -> bool {
    (0..words.len())
        .any(|at| is_bare(words.get(at), DYNAMIC_SQL) && !is_a_trigger_clause(words, at))
}

/// The bodies `statement` defines that run dynamic SQL, by owner (see [`body_owners`]), its
/// bodies read whichever way they are quoted.
fn dynamic_bodies(statement: &str) -> Vec<String> {
    let mut out: Vec<String> = body_texts(statement)
        .iter()
        .map(|text| sql_words(text))
        .filter(|words| runs_dynamic_sql(words))
        .flat_map(|words| body_owners(&words))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// What one statement of a later migration does with dynamic SQL that only a listed one may: a
/// function, procedure or `DO` body that runs `EXECUTE`, or a body or trigger that names one of
/// `executors`, the functions any migration defines that run it.
fn dynamic_sql_findings(statement: &str, executors: &BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = dynamic_bodies(statement)
        .into_iter()
        .map(|owner| format!("`{owner}` runs dynamic SQL ({DYNAMIC_SQL})"))
        .collect();
    for text in body_texts(statement) {
        let words = sql_words(&text);
        let owners = body_owners(&words);
        let runs_later = !owners.is_empty()
            || (0..words.len()).any(|at| {
                running_object_at(&words, at).is_some_and(|(_, object)| object == "TRIGGER")
            });
        if !runs_later {
            continue;
        }
        let named = words
            .iter()
            .filter_map(|word| table_name(Some(word)))
            .filter(|name| executors.contains(name) && !owners.contains(name));
        for executor in named {
            out.push(format!("names `{executor}`, which runs dynamic SQL"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The words that may stand between `CREATE` and `VIEW`. A materialized view is not among them:
/// no statement writes through one.
const VIEW_MODIFIERS: [&str; 5] = ["OR", "REPLACE", "TEMP", "TEMPORARY", "RECURSIVE"];

/// The words the tables a query reads follow: `TABLE t` reads as `SELECT * FROM t`.
const READ_FROM: [&str; 3] = ["FROM", "JOIN", "TABLE"];

/// Each commission table a view that `words` define reads, at any depth of its query: the
/// table after each of [`READ_FROM`] up to the end of the view's statement. A simple view
/// writes through to the table it reads.
fn commission_views(words: &[SqlWord]) -> Vec<String> {
    let mut out = Vec::new();
    for at in 0..words.len() {
        if !is_bare(words.get(at), "CREATE") {
            continue;
        }
        let mut view = at + 1;
        while VIEW_MODIFIERS
            .iter()
            .any(|modifier| is_bare(words.get(view), modifier))
        {
            view += 1;
        }
        if !is_bare(words.get(view), "VIEW") {
            continue;
        }
        let end = (view..words.len())
            .find(|index| words[*index] == SqlWord::Other(';'))
            .unwrap_or(words.len());
        for reads in view..end {
            if !READ_FROM
                .iter()
                .any(|keyword| is_bare(words.get(reads), keyword))
            {
                continue;
            }
            let start = if is_bare(words.get(reads + 1), "ONLY") {
                reads + 2
            } else {
                reads + 1
            };
            let table = table_name(words.get(table_name_at(words, start)));
            if let Some(table) = table.filter(|table| is_a_commission_table(table)) {
                out.push(table);
            }
        }
    }
    out
}

/// Each view over a commission table that one statement of a later migration defines, in its
/// text or in a body it holds, whichever way the body is quoted.
fn view_findings(statement: &str) -> Vec<String> {
    let mut out: Vec<String> = body_texts(statement)
        .iter()
        .flat_map(|text| commission_views(&sql_words(text)))
        .map(|table| format!("defines a view over `{table}`"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// An escape string (`E'…'`) in a finding: PostgreSQL decodes its backslash escapes.
const ESCAPE_STRING: &str = "an escape string (E'…')";

/// A Unicode-escape string (`U&'…'`) in a finding: PostgreSQL decodes its `\XXXX` escapes.
const UNICODE_STRING: &str = "a Unicode-escape string (U&'…')";

/// What decodes the single-quoted string opening at `at`, by the prefix written right before
/// its quote and ending `words` (the words before it at its level): `E` for an escape string,
/// `U` and `&` for a Unicode-escape string; with how many of `words` the prefix takes.
fn decoding_prefix(
    characters: &[char],
    at: usize,
    words: &[String],
) -> Option<(&'static str, usize)> {
    let before = |back: usize| at.checked_sub(back).map(|index| characters[index]);
    let word = |back: usize| {
        words
            .len()
            .checked_sub(back)
            .map(|index| words[index].as_str())
    };
    let escape = matches!(before(1), Some('E' | 'e'))
        && word(1).is_some_and(|prefix| prefix.eq_ignore_ascii_case("e"));
    let unicode = before(1) == Some('&')
        && matches!(before(2), Some('U' | 'u'))
        && word(1) == Some("&")
        && word(2).is_some_and(|prefix| prefix.eq_ignore_ascii_case("u"));
    if escape {
        Some((ESCAPE_STRING, 1))
    } else if unicode {
        Some((UNICODE_STRING, 2))
    } else {
        None
    }
}

/// Whether `words`, the words before a string at its level, end where a body follows: `AS`,
/// `DO`, or `DO LANGUAGE` and the language's name.
fn introduces_a_body(words: &[String]) -> bool {
    let is = |back: usize, expected: &str| {
        words
            .len()
            .checked_sub(back)
            .is_some_and(|index| words[index].eq_ignore_ascii_case(expected))
    };
    is(1, "AS") || is(1, "DO") || (is(2, "LANGUAGE") && is(3, "DO"))
}

/// The text inside the quoted run from `at` to `end` that opens with `opener` characters and
/// closes with the same ones, when it closes.
fn quoted_text(characters: &[char], at: usize, end: usize, opener: usize) -> String {
    let start = (at + opener).min(end);
    let closes =
        end >= start + opener && characters[end - opener..end] == characters[at..at + opener];
    let close = if closes { end - opener } else { end };
    characters[start..close].iter().collect()
}

/// What decodes each body `sql` writes in a decoded string, at any depth: a string right after
/// `AS` or `DO` written as an escape or Unicode-escape string, in the text and in each
/// dollar-quoted or plain string's text read as SQL. PostgreSQL decodes such a body before it
/// reads it, so its statements may be spelled in escapes no scan here reads.
fn decoded_bodies(sql: &str, out: &mut Vec<&'static str>) {
    let characters: Vec<char> = sql.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut at = 0;
    while at < characters.len() {
        let character = characters[at];
        if let Some(end) = past_a_quote(&characters, at) {
            match character {
                '\'' => {
                    match decoding_prefix(&characters, at, &words) {
                        Some((decoder, taken)) => {
                            if introduces_a_body(&words[..words.len() - taken]) {
                                out.push(decoder);
                            }
                        }
                        None => {
                            let inner = quoted_text(&characters, at, end, 1).replace("''", "'");
                            decoded_bodies(&inner, out);
                        }
                    }
                    words.push("'".to_owned());
                }
                '$' => {
                    let tag = (at + 1..end)
                        .find(|index| characters[*index] == '$')
                        .map_or(1, |tag_end| tag_end + 1 - at);
                    decoded_bodies(&quoted_text(&characters, at, end, tag), out);
                    words.push("$".to_owned());
                }
                '"' => words.push("\"".to_owned()),
                _ => {}
            }
            at = end;
        } else if character.is_alphanumeric() || character == '_' {
            let start = at;
            while at < characters.len()
                && (characters[at].is_alphanumeric()
                    || characters[at] == '_'
                    || characters[at] == '$')
            {
                at += 1;
            }
            words.push(characters[start..at].iter().collect());
        } else {
            if !character.is_whitespace() {
                words.push(character.to_string());
            }
            at += 1;
        }
    }
}

/// Each way one statement of a later migration writes a body in a decoded string.
fn decoded_body_findings(statement: &str) -> Vec<String> {
    let mut decoders = Vec::new();
    decoded_bodies(statement, &mut decoders);
    decoders.sort_unstable();
    decoders.dedup();
    decoders
        .into_iter()
        .map(|decoder| format!("writes a body as {decoder}"))
        .collect()
}

/// The names that reach `standard_conforming_strings`: set off, it makes PostgreSQL decode
/// backslash escapes in every string, a plain body's too.
const STRING_SETTING_DOORS: [&str; 3] =
    ["standard_conforming_strings", "set_config", "pg_settings"];

/// What one statement of a later migration names that could change how its strings decode: one
/// of [`STRING_SETTING_DOORS`], bare or quoted, in its text or its strings'; or a
/// Unicode-escaped identifier, which can spell any name.
fn string_setting_findings(statement: &str) -> Vec<String> {
    let mut out: Vec<String> = body_texts(statement)
        .iter()
        .flat_map(|text| sql_words(text))
        .filter_map(|word| table_name(Some(&word)))
        .filter(|name| STRING_SETTING_DOORS.contains(&name.as_str()))
        .map(|name| format!("names `{name}`, which sets how strings decode"))
        .collect();
    if holds_a_unicode_identifier(statement) {
        out.push("holds a Unicode-escaped identifier".to_owned());
    }
    out.sort();
    out.dedup();
    out
}

/// What breaks the migration rule in `files` (each by file name, with its SQL): each statement
/// of a migration stamped after [`LAST_UNREAD_MIGRATION`] and not in `allowed` that
/// [`running_write_findings`] refuses, against the writer functions any migration defines, or
/// that [`dynamic_sql_findings`] refuses, against the functions any migration defines that run
/// dynamic SQL, or that defines a view over a commission table, writes a body in a decoded
/// string or names a way to change how strings decode; and a migration without a numeric stamp.
fn migration_findings(files: &[(String, String)], allowed: &[&str]) -> Vec<String> {
    let writers: BTreeSet<String> = files
        .iter()
        .flat_map(|(_, sql)| sql_statements(sql))
        .flat_map(|statement| writer_functions(&statement))
        .collect();
    let executors: BTreeSet<String> = files
        .iter()
        .flat_map(|(_, sql)| sql_statements(sql))
        .flat_map(|statement| dynamic_bodies(&statement))
        .filter(|owner| owner != DO_BLOCK)
        .collect();
    let mut out = Vec::new();
    for (file, sql) in files {
        let shown = format!("adapter-pg/migrations/{file}");
        let stamp = file
            .split_once('_')
            .and_then(|(version, _)| version.parse::<u64>().ok());
        let Some(stamp) = stamp else {
            out.push(format!("{shown}: no numeric version stamp"));
            continue;
        };
        if stamp <= LAST_UNREAD_MIGRATION || allowed.contains(&file.as_str()) {
            continue;
        }
        for statement in sql_statements(sql) {
            let mut findings: Vec<String> = running_write_findings(&statement, &writers)
                .into_iter()
                .chain(dynamic_sql_findings(&statement, &executors))
                .chain(view_findings(&statement))
                .chain(decoded_body_findings(&statement))
                .chain(string_setting_findings(&statement))
                .collect();
            findings.sort();
            findings.dedup();
            for finding in findings {
                out.push(format!("{shown}: {finding}"));
            }
        }
    }
    out
}

/// Every migration of the PostgreSQL adapter, by file name, with its SQL.
fn migrations() -> Vec<(String, String)> {
    let folder = adapter_pg().join("migrations");
    let entries = std::fs::read_dir(&folder).expect("a readable migrations folder");
    let mut out: Vec<(String, String)> = entries
        .map(|entry| entry.expect("a migrations entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .map(|path| {
            let sql = std::fs::read_to_string(&path).expect("a readable migration");
            (relative(&path, &folder), sql)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn no_later_migration_defines_a_function_or_trigger_that_writes_a_commission_table() {
    let files = migrations();

    let findings = migration_findings(&files, &COMMISSION_WRITING_MIGRATIONS);

    assert_eq!(findings, Vec::<String>::new());
    assert!(
        files.len() > 40,
        "the scan reads the migrations: {}",
        files.len()
    );
    let later = "20261023000000_later.sql";
    let earlier_writer = (
        "20261022132603_sync.sql".to_owned(),
        "CREATE FUNCTION sync_title() RETURNS trigger AS $$ BEGIN UPDATE commission SET title = NEW.name WHERE node_id = NEW.id; RETURN NEW; END $$ LANGUAGE plpgsql;".to_owned(),
    );
    let earlier_executor = (
        "20261022132604_run.sql".to_owned(),
        "CREATE FUNCTION run_sql(statement text) RETURNS void AS $$ BEGIN EXECUTE statement; END $$ LANGUAGE plpgsql;".to_owned(),
    );
    let refused = [
        "CREATE OR REPLACE FUNCTION archive_all() RETURNS void AS $$\nBEGIN\n  UPDATE commission SET archived_at = now();\nEND\n$$ LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS 'UPDATE commission SET archived_at = now()' LANGUAGE sql;",
        "CREATE FUNCTION archive_all() RETURNS void LANGUAGE sql BEGIN ATOMIC SELECT 1; DELETE FROM commission_tab; END;",
        "CREATE TRIGGER stamp BEFORE UPDATE ON commission FOR EACH ROW EXECUTE FUNCTION touch();",
        "CREATE TRIGGER sync AFTER UPDATE ON node FOR EACH ROW EXECUTE FUNCTION sync_title();",
        "CREATE OR REPLACE FUNCTION commission_columns_follow_their_node() RETURNS trigger AS $$ BEGIN NEW.title := 'x'; RETURN NEW; END $$ LANGUAGE plpgsql;",
        "DO $$ BEGIN EXECUTE 'CREATE FUNCTION leak() RETURNS void AS ''UPDATE commission SET archived_at = now()'' LANGUAGE sql'; END $$;",
        "CREATE RULE archive AS ON UPDATE TO workflow DO ALSO UPDATE commission SET archived_at = now();",
        "CREATE RULE keep AS ON DELETE TO commission DO INSTEAD NOTHING;",
        "CREATE FUNCTION archive_all() RETURNS void AS $$ UPDATE U&\"\\0063ommission\" SET archived_at = now() $$ LANGUAGE sql;",
        "CREATE FUNCTION wipe_slots() RETURNS void AS $body$ DELETE FROM public.\"commission_slot\" $body$ LANGUAGE sql;",
        "create constraint trigger late after insert on commission_changelog deferrable for each row execute procedure sync_title();",
        "CREATE /* the card */ FUNCTION archive_all() RETURNS void AS $$ UPDATE -- the card\n commission SET archived_at = now() $$ LANGUAGE sql;",
        "CREATE FUNCTION archive_all() RETURNS void AS E'SELECT \\';\\'; UPDATE commission SET archived_at = now()' LANGUAGE sql;",
        "CREATE PROCEDURE archive_all() LANGUAGE plpgsql AS $$ BEGIN EXECUTE 'UPDATE ' || quote_ident('commission') || ' SET archived_at = now()'; END $$;",
        "DO $$ BEGIN EXECUTE format('UPDATE %I SET archived_at = now()', 'commission'); END $$;",
        "CREATE FUNCTION wipe_tabs() RETURNS void AS $fn$ BEGIN EXECUTE 'DELETE FROM ' || 'commission_tab'; END $fn$ LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS 'BEGIN EXECUTE ''UPDATE '' || ''commission'' || '' SET archived_at = now()''; END' LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS $$ DECLARE function text := 'UPDATE ' || 'commission'; BEGIN EXECUTE function; END $$ LANGUAGE plpgsql;",
        "CREATE FUNCTION archived() RETURNS SETOF commission AS $$ BEGIN RETURN QUERY EXECUTE 'SELECT * FROM ' || 'commission'; END $$ LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS $$ DECLARE dash text := '--'; BEGIN EXECUTE 'UPD' || 'ATE commission'; END $$ LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS $$ BEGIN PERFORM run_sql('UPD' || 'ATE commission SET archived_at = now()'); END $$ LANGUAGE plpgsql;",
        "CREATE TRIGGER rerun AFTER UPDATE ON node FOR EACH ROW EXECUTE FUNCTION run_sql();",
        "CREATE VIEW heron_v AS SELECT id, node_id, archived_at FROM commission;",
        "CREATE VIEW heron_v AS TABLE commission;",
        "CREATE OR REPLACE TEMP VIEW open_slots AS SELECT * FROM ONLY public.\"commission_slot\";",
        "CREATE VIEW open_nodes AS SELECT * FROM node WHERE id IN (SELECT node_id FROM commission);",
        "CREATE VIEW cards AS SELECT node.id FROM node JOIN commission ON commission.node_id = node.id;",
        "DO $$ BEGIN CREATE VIEW heron_v AS SELECT * FROM commission; END $$;",
        "CREATE FUNCTION heron_escape_unarchive() RETURNS trigger AS E'BEGIN \\x55PDATE commission SET archived_at = NULL WHERE node_id = NEW.id; RETURN NEW; END' LANGUAGE plpgsql;",
        "CREATE FUNCTION archive_all() RETURNS void AS U&'\\0055PDATE commission SET archived_at = now()' LANGUAGE sql;",
        "CREATE PROCEDURE archive_all() LANGUAGE sql AS /* the /* nested */ body */ u&'!0055PDATE commission SET archived_at = now()' UESCAPE '!';",
        "CREATE FUNCTION archive_all() RETURNS void AS /* a /* b */ c */ e'\\x55PDATE commission SET archived_at = now()' LANGUAGE sql;",
        "DO E'BEGIN \\x55PDATE commission SET archived_at = NULL; END';",
        "DO LANGUAGE plpgsql e'BEGIN \\x55PDATE commission SET archived_at = NULL; END';",
        "DO $$ BEGIN CREATE FUNCTION leak() RETURNS void AS E'\\x55PDATE commission SET archived_at = now()' LANGUAGE sql; END $$;",
        "DO 'BEGIN CREATE FUNCTION leak() RETURNS void AS E''\\x55PDATE commission SET archived_at = now()'' LANGUAGE sql; END';",
        "CREATE FUNCTION archive_all() RETURNS void AS 'UPDATE commi'\n  'ssion SET archived_at = now()' LANGUAGE sql;",
        "SET standard_conforming_strings = off;",
        "SELECT set_config(E'\\x73tandard_conforming_strings', 'off', false);",
        "UPDATE pg_settings SET setting = 'off' WHERE name = E'\\x73tandard_conforming_strings';",
        "SET U&\"\\0073tandard_conforming_strings\" = off;",
    ];
    for sql in refused {
        let planted = [
            earlier_writer.clone(),
            earlier_executor.clone(),
            (later.to_owned(), sql.to_owned()),
        ];

        let found = migration_findings(&planted, &COMMISSION_WRITING_MIGRATIONS);

        assert!(!found.is_empty(), "{sql}");
        assert!(
            found
                .iter()
                .all(|finding| finding
                    .starts_with("adapter-pg/migrations/20261023000000_later.sql: ")),
            "{sql}: {found:?}"
        );
        let listed = migration_findings(&planted, &[later]);
        assert_eq!(
            listed,
            Vec::<String>::new(),
            "a listed migration is read past: {sql}"
        );
    }
    let kept = [
        "DO $$ BEGIN UPDATE commission SET archived_at = now() WHERE false; END $$;",
        "UPDATE commission SET archived_at = NULL WHERE false;\nCREATE FUNCTION tidy() RETURNS void AS $$ DELETE FROM workflow WHERE false $$ LANGUAGE sql;",
        "CREATE FUNCTION count_open() RETURNS bigint AS $$ SELECT count(*) FROM commission WHERE archived_at IS NULL $$ LANGUAGE sql;",
        "CREATE TRIGGER name_node BEFORE INSERT ON node FOR EACH ROW EXECUTE FUNCTION node_name_known();",
        "CREATE FUNCTION node_unmoved() RETURNS trigger AS $$ BEGIN IF NEW.parent_id = OLD.parent_id THEN RETURN NEW; END IF; RAISE EXCEPTION 'moved'; END $$ LANGUAGE plpgsql;",
        "CREATE INDEX commission_deadline ON commission (deadline);",
        "CREATE TRIGGER name_node AFTER UPDATE ON node FOR EACH ROW EXECUTE PROCEDURE node_name_known();",
        "DO $$ BEGIN CREATE TRIGGER name_node AFTER INSERT ON node FOR EACH ROW EXECUTE FUNCTION node_name_known(); END $$;",
        "-- Executes nothing at run time.\nCREATE FUNCTION tidy() RETURNS void AS $$ DELETE FROM workflow WHERE false $$ LANGUAGE sql;",
        "CREATE MATERIALIZED VIEW open_commissions AS SELECT id FROM commission WHERE archived_at IS NULL;",
        "CREATE VIEW open_nodes AS SELECT id, name FROM node;",
        "CREATE FUNCTION node_unmoved() RETURNS trigger AS $$ BEGIN IF NEW.parent_id = OLD.parent_id THEN RETURN NEW; END IF; RAISE EXCEPTION E'moved\\n'; END $$ LANGUAGE plpgsql;",
        "ALTER TABLE bug_report ADD CONSTRAINT no_backslash CHECK (position(E'\\\\' in page) = 0);",
        "CREATE FUNCTION tidy() RETURNS void AS 'DELETE FROM work'\n  'flow WHERE false' LANGUAGE sql;",
    ];
    for sql in kept {
        let planted = [
            earlier_writer.clone(),
            earlier_executor.clone(),
            (later.to_owned(), sql.to_owned()),
        ];

        let found = migration_findings(&planted, &COMMISSION_WRITING_MIGRATIONS);

        assert_eq!(found, Vec::<String>::new(), "{sql}");
    }
    let herons_plant = "-- A trigger on another table that un-archives a commission by dynamic SQL.\nCREATE FUNCTION heron_unarchive() RETURNS trigger AS $$\nBEGIN\n    EXECUTE format('UPDATE %I SET archived_at = NULL WHERE node_id = $1', 'commission') USING NEW.id;\n    RETURN NEW;\nEND\n$$ LANGUAGE plpgsql;\n\nCREATE TRIGGER heron_unarchive AFTER UPDATE ON node FOR EACH ROW EXECUTE FUNCTION heron_unarchive();\n\n-- The same, with the statement concatenated.\nCREATE FUNCTION heron_unarchive_two() RETURNS trigger AS $$\nBEGIN\n    EXECUTE 'UPDATE ' || 'commission' || ' SET archived_at = NULL WHERE node_id = $1' USING NEW.id;\n    RETURN NEW;\nEND\n$$ LANGUAGE plpgsql;\n";
    let planted = [(later.to_owned(), herons_plant.to_owned())];
    let found = migration_findings(&planted, &COMMISSION_WRITING_MIGRATIONS);
    let shown = "adapter-pg/migrations/20261023000000_later.sql";
    let expected = [
        format!("{shown}: `heron_unarchive_two` runs dynamic SQL (EXECUTE)"),
        format!("{shown}: `heron_unarchive` runs dynamic SQL (EXECUTE)"),
        format!("{shown}: names `heron_unarchive`, which runs dynamic SQL"),
    ];
    for finding in &expected {
        assert!(found.contains(finding), "{finding}: {found:?}");
    }
    let herons_view = "-- A view over the commission table, written by a trigger on another table.\nCREATE VIEW heron_v AS SELECT id, node_id, archived_at FROM commission;\n\nCREATE FUNCTION heron_view_unarchive() RETURNS trigger AS $$\nBEGIN\n    UPDATE heron_v SET archived_at = NULL WHERE node_id = NEW.id;\n    RETURN NEW;\nEND\n$$ LANGUAGE plpgsql;\n\nCREATE TRIGGER heron_view_unarchive AFTER UPDATE ON node FOR EACH ROW EXECUTE FUNCTION heron_view_unarchive();\n";
    let herons_escape = "-- A function body in an escape string, its statement's first letter escaped.\nCREATE FUNCTION heron_escape_unarchive() RETURNS trigger AS E'BEGIN \\x55PDATE commission SET archived_at = NULL WHERE node_id = NEW.id; RETURN NEW; END' LANGUAGE plpgsql;\n\nCREATE TRIGGER heron_escape_unarchive AFTER UPDATE ON node FOR EACH ROW EXECUTE FUNCTION heron_escape_unarchive();\n";
    let literal_plants = [
        (
            herons_view,
            format!("{shown}: defines a view over `commission`"),
        ),
        (
            herons_escape,
            format!("{shown}: writes a body as {ESCAPE_STRING}"),
        ),
    ];
    for (plant, finding) in literal_plants {
        let planted = [(later.to_owned(), plant.to_owned())];

        let found = migration_findings(&planted, &COMMISSION_WRITING_MIGRATIONS);

        assert_eq!(found, [finding], "{plant}");
    }
    let earlier = [(earlier_writer.0.clone(), refused[0].to_owned())];
    assert_eq!(
        migration_findings(&earlier, &COMMISSION_WRITING_MIGRATIONS),
        Vec::<String>::new(),
        "a migration stamped before the rule is history"
    );
    let unstamped = [("later.sql".to_owned(), refused[0].to_owned())];
    assert!(!migration_findings(&unstamped, &COMMISSION_WRITING_MIGRATIONS).is_empty());
}

// The gated changelog constructors are named only in the rows file and the act file.

/// A new changelog entry, whose gated constructors only the act file names.
const NEW_CHANGELOG_ENTRY: &str = "NewChangelogEntry";

/// An entry of an open kind, the only shape a note rides beside a payload.
const OPEN_CHANGELOG_ENTRY: &str = "OpenChangelogEntry";

/// The new entry's open constructors, any use case's to call: every other is gated.
const OPEN_CONSTRUCTORS: [&str; 2] = ["event", "note"];

/// The open entry's one method, which attaches a note.
const OPEN_ENTRY_NOTE: &str = "with_note";

/// The new entry's accessors, the only items of the rows file that reach an entry's fields.
const ACCESSORS: [&str; 6] = [
    "actor_id",
    "commission_id",
    "created_at",
    "kind",
    "note_text",
    "payload",
];

/// The new entry's private helper, named only by the gated and the open constructors.
const ENTRY_HELPER: &str = "by";

/// Every item of the entry types' inherent impls that isn't gated, by its type and name,
/// pinned to its visibility and signature: the accessors, the helper, the open constructors
/// and the open entry's note. Every other item there that isn't private is gated.
const ENTRY_SURFACE: [(&str, &str, &str); 10] = [
    (
        NEW_CHANGELOG_ENTRY,
        "commission_id",
        "pub fn commission_id(&self) -> CommissionId",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "kind",
        "pub fn kind(&self) -> ChangelogEntryKind",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "actor_id",
        "pub fn actor_id(&self) -> Option<&UserId>",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "payload",
        "pub fn payload(&self) -> &Value",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "note_text",
        "pub fn note_text(&self) -> Option<&str>",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "created_at",
        "pub fn created_at(&self) -> DateTimeUtc",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        ENTRY_HELPER,
        "fn by(commission: CommissionId, kind: ChangelogEntryKind, actor: UserId, payload: Value, at: DateTimeUtc) -> Self",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "event",
        "pub fn event(commission: CommissionId, kind: OpenKind, actor: UserId, payload: Value, at: DateTimeUtc) -> OpenChangelogEntry",
    ),
    (
        NEW_CHANGELOG_ENTRY,
        "note",
        "pub fn note(commission: CommissionId, actor: UserId, text: String, at: DateTimeUtc) -> Self",
    ),
    (
        OPEN_CHANGELOG_ENTRY,
        OPEN_ENTRY_NOTE,
        "pub fn with_note(self, text: String) -> Self",
    ),
];

/// The items of the rows file that build an entry by hand, with a struct literal or pattern
/// of either entry type: the helper, the open constructors, the system's Late mark (which has
/// no actor) and the open entry's note.
const ENTRY_BUILDERS: [&str; 5] = [
    "NewChangelogEntry::by",
    "NewChangelogEntry::event",
    "NewChangelogEntry::late",
    "NewChangelogEntry::note",
    "OpenChangelogEntry::with_note",
];

/// The gated constructors, as the domain defines them today.
const GATED_CONSTRUCTORS: [&str; 13] = [
    "created",
    "deadline_cleared",
    "deadline_set",
    "delayed_cleared",
    "delayed_set",
    "direction_cleared",
    "direction_set",
    "dispute_cleared",
    "disputed",
    "late",
    "lifecycle_moved",
    "maturity_rated",
    "renamed",
];

/// The domain file that defines the entry types and their constructors, from the crates folder.
const ROWS_FILE: &str = "workflow/src/elements/commission/changelog/rows.rs";

/// The files that may name a gated constructor, from the crates folder: the rows file that
/// defines them, and the act file.
const GATED_CONSTRUCTOR_FILES: [&str; 2] = [ROWS_FILE, "application/src/den/act.rs"];

/// Keywords that open an item, which no body in the rows file may hold.
const ITEM_KEYWORDS: [&str; 12] = [
    "const",
    "enum",
    "fn",
    "impl",
    "macro_rules",
    "mod",
    "static",
    "struct",
    "trait",
    "type",
    "union",
    "use",
];

/// The entry types: only the rows file declares them or gives them an inherent item.
const ENTRY_TYPES: [&str; 2] = [NEW_CHANGELOG_ENTRY, OPEN_CHANGELOG_ENTRY];

/// The attributes the rows file may carry beside docs, at any depth: its structs' derive and
/// its unit tests' `cfg`. Any other derive or attribute macro generates code no scan here
/// reads, such as a `From` of an entry's fields, or a deserializer of an entry or of a remote
/// copy of one.
const ROWS_ATTRIBUTES: [&str; 2] = ["#[derive(Debug)]", "#[cfg(test)]"];

/// The one attribute each entry type carries beside its docs.
const ENTRY_ATTRIBUTES: [&str; 1] = ["#[derive(Debug)]"];

/// The rows file's one module, its unit tests: a child module sees an entry's private fields.
const ROWS_TESTS: &str = "tests";

/// The attributes the unit-test module carries beside its docs.
const ROWS_TESTS_ATTRIBUTES: [&str; 1] = ["#[cfg(test)]"];

fn changelog_rows_file() -> PathBuf {
    crates().join(ROWS_FILE)
}

/// `text` with its whitespace and trailing commas dropped, so two spellings of one signature
/// compare equal.
fn squeezed(text: &str) -> String {
    let bare: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    bare.replace(",)", ")").replace(",>", ">")
}

/// The names a file gives each entry type, its aliases included.
struct EntryNames {
    new: BTreeSet<String>,
    open: BTreeSet<String>,
}

impl EntryNames {
    fn of(rows: &syn::File) -> Self {
        Self {
            new: aliases_in_file(&[NEW_CHANGELOG_ENTRY], rows),
            open: aliases_in_file(&[OPEN_CHANGELOG_ENTRY], rows),
        }
    }

    /// The entry type `typ` names, by its own name, or `None` for another type.
    fn owner(&self, typ: &Type) -> Option<&'static str> {
        let name = type_name(typ);
        if self.new.contains(&name) {
            Some(NEW_CHANGELOG_ENTRY)
        } else if self.open.contains(&name) {
            Some(OPEN_CHANGELOG_ENTRY)
        } else {
            None
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.new.contains(name) || self.open.contains(name)
    }
}

/// One item of an entry type's inherent impl: the type, the item's name, whether it is
/// private (`pub(crate)`, `pub(super)` and `pub(in …)` are not), and how it is written (a
/// function's visibility and signature, any other item whole).
struct EntryImplItem {
    owner: &'static str,
    name: String,
    private: bool,
    written: String,
}

/// Every item of every inherent impl of an entry type in `rows`, inline modules included.
fn entry_impl_items(rows: &syn::File) -> Vec<EntryImplItem> {
    let names = EntryNames::of(rows);
    every_item(&rows.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Impl(implementation) if implementation.trait_.is_none() => names
                .owner(&implementation.self_ty)
                .map(|owner| (owner, implementation)),
            _ => None,
        })
        .flat_map(|(owner, implementation)| {
            implementation
                .items
                .iter()
                .map(move |impl_item| entry_impl_item(owner, impl_item))
        })
        .collect()
}

fn entry_impl_item(owner: &'static str, impl_item: &ImplItem) -> EntryImplItem {
    let is_private = |vis: &Visibility| matches!(vis, Visibility::Inherited);
    let whole = impl_item.to_token_stream().to_string();
    let (name, private, written) = match impl_item {
        ImplItem::Fn(function) => {
            let mut signature = function.vis.to_token_stream();
            signature.extend(function.sig.to_token_stream());
            (
                unraw(&function.sig.ident),
                is_private(&function.vis),
                signature.to_string(),
            )
        }
        ImplItem::Const(constant) => (unraw(&constant.ident), is_private(&constant.vis), whole),
        ImplItem::Type(alias) => (unraw(&alias.ident), is_private(&alias.vis), whole),
        _ => (whole.clone(), true, whole),
    };
    EntryImplItem {
        owner,
        name,
        private,
        written,
    }
}

/// The pinned spelling of `item`, when [`ENTRY_SURFACE`] lists it.
fn pinned(item: &EntryImplItem) -> Option<&'static str> {
    ENTRY_SURFACE
        .iter()
        .find(|(owner, name, _)| *owner == item.owner && *name == item.name)
        .map(|(_, _, written)| *written)
}

/// The gated constructors `rows` defines: every item of an entry type's inherent impl that
/// isn't private and isn't on [`ENTRY_SURFACE`], whatever it answers.
fn gated_constructors(rows: &syn::File) -> BTreeSet<String> {
    entry_impl_items(rows)
        .into_iter()
        .filter(|item| !item.private && pinned(item).is_none())
        .map(|item| item.name)
        .collect()
}

/// What breaks the entry types' pinned surface in `rows`: a listed item spelled otherwise, a
/// private item no list names, and a listed item that is missing.
fn entry_surface_findings(rows: &syn::File) -> Vec<String> {
    let items = entry_impl_items(rows);
    let mut out = Vec::new();
    for item in &items {
        let EntryImplItem { owner, name, .. } = item;
        match pinned(item) {
            Some(written) if squeezed(written) != squeezed(&item.written) => out.push(format!(
                "{ROWS_FILE}: `{owner}::{name}` is `{}`, not `{written}`",
                item.written
            )),
            None if item.private => out.push(format!(
                "{ROWS_FILE}: `{owner}::{name}` is a private item no list names"
            )),
            _ => {}
        }
    }
    for (owner, name, _) in ENTRY_SURFACE {
        let present = items
            .iter()
            .any(|item| item.owner == owner && item.name == name);
        if !present {
            out.push(format!("{ROWS_FILE}: `{owner}::{name}` is missing"));
        }
    }
    out
}

/// A unit of the rows file a seal finding names: an item, an impl's header, or an item of an
/// impl or a trait (`Type::name`, or `<Type as Trait>::name` in a trait impl). `body` is what
/// runs: a function's block, a constant's value, a macro's input.
struct RowsUnit {
    label: String,
    self_is_entry: bool,
    tokens: TokenStream,
    body: Option<TokenStream>,
}

fn impl_item_body(impl_item: &ImplItem) -> Option<TokenStream> {
    match impl_item {
        ImplItem::Fn(function) => Some(function.block.to_token_stream()),
        ImplItem::Const(constant) => Some(constant.expr.to_token_stream()),
        ImplItem::Macro(called) => Some(called.mac.tokens.clone()),
        ImplItem::Verbatim(tokens) => Some(tokens.clone()),
        _ => None,
    }
}

fn impl_item_name(impl_item: &ImplItem) -> String {
    match impl_item {
        ImplItem::Fn(function) => unraw(&function.sig.ident),
        ImplItem::Const(constant) => unraw(&constant.ident),
        ImplItem::Type(alias) => unraw(&alias.ident),
        other => other.to_token_stream().to_string(),
    }
}

/// Every unit of `rows`, inline modules included.
fn rows_units(rows: &syn::File, names: &EntryNames) -> Vec<RowsUnit> {
    let mut out = Vec::new();
    for item in every_item(&rows.items) {
        match item {
            Item::Mod(_) => {}
            Item::Impl(implementation) => {
                let owner = names.owner(&implementation.self_ty);
                let self_name =
                    owner.map_or_else(|| type_name(&implementation.self_ty), str::to_owned);
                let base = match &implementation.trait_ {
                    Some((_, trait_path, _)) => {
                        let trait_name = squeezed(&trait_path.to_token_stream().to_string());
                        format!("<{self_name} as {trait_name}>")
                    }
                    None => self_name,
                };
                let mut header = implementation.self_ty.to_token_stream();
                if let Some((_, trait_path, _)) = &implementation.trait_ {
                    header.extend(trait_path.to_token_stream());
                }
                header.extend(implementation.generics.to_token_stream());
                for attribute in &implementation.attrs {
                    header.extend(attribute.to_token_stream());
                }
                out.push(RowsUnit {
                    label: format!("impl {base}"),
                    self_is_entry: owner.is_some(),
                    tokens: header,
                    body: None,
                });
                for impl_item in &implementation.items {
                    out.push(RowsUnit {
                        label: format!("{base}::{}", impl_item_name(impl_item)),
                        self_is_entry: owner.is_some(),
                        tokens: impl_item.to_token_stream(),
                        body: impl_item_body(impl_item),
                    });
                }
            }
            Item::Trait(tr) => {
                for trait_item in &tr.items {
                    let (name, body) = match trait_item {
                        TraitItem::Fn(method) => (
                            unraw(&method.sig.ident),
                            method.default.as_ref().map(ToTokens::to_token_stream),
                        ),
                        TraitItem::Const(constant) => (
                            unraw(&constant.ident),
                            constant
                                .default
                                .as_ref()
                                .map(|(_, value)| value.to_token_stream()),
                        ),
                        other => (
                            other.to_token_stream().to_string(),
                            Some(other.to_token_stream()),
                        ),
                    };
                    out.push(RowsUnit {
                        label: format!("{}::{name}", tr.ident),
                        self_is_entry: false,
                        tokens: trait_item.to_token_stream(),
                        body,
                    });
                }
                let mut header = tr.supertraits.to_token_stream();
                header.extend(tr.generics.to_token_stream());
                out.push(RowsUnit {
                    label: format!("trait {}", tr.ident),
                    self_is_entry: false,
                    tokens: header,
                    body: None,
                });
            }
            other => {
                let body = match other {
                    Item::Fn(function) => Some(function.block.to_token_stream()),
                    Item::Const(constant) => Some(constant.expr.to_token_stream()),
                    Item::Static(stat) => Some(stat.expr.to_token_stream()),
                    Item::Verbatim(tokens) => Some(tokens.clone()),
                    _ => None,
                };
                out.push(RowsUnit {
                    label: item_name(other),
                    self_is_entry: false,
                    tokens: other.to_token_stream(),
                    body,
                });
            }
        }
    }
    out
}

/// Each name `tokens` holds as a path's last segment (`T::c`, `<T>::c`, `Self::c`).
fn path_last_segments(tokens: TokenStream) -> Vec<String> {
    let mut out = Vec::new();
    for level in levels(tokens) {
        for at in 2..level.len() {
            if let Some(name) = ident_at(&level, at)
                && path_separator_at(&level, at - 2)
            {
                out.push(name);
            }
        }
    }
    out
}

/// Whether `tokens` build or take apart an entry by hand: an entry type's name (or `Self` when
/// `self_is_entry`) right before braces or parentheses, unless it is being declared or
/// answered (after `impl`, `for`, `struct`, `enum`, `union`, `trait` or `>`).
fn builds_an_entry(tokens: TokenStream, names: &EntryNames, self_is_entry: bool) -> bool {
    levels(tokens).iter().any(|level| {
        (0..level.len()).any(|at| {
            let Some(name) = ident_at(level, at) else {
                return false;
            };
            let entry = names.contains(&name) || (self_is_entry && name == "Self");
            let opened = matches!(
                level.get(at + 1),
                Some(TokenTree::Group(group))
                    if matches!(group.delimiter(), Delimiter::Brace | Delimiter::Parenthesis)
            );
            let declared = at > 0
                && (punct_at(level, at - 1, '>')
                    || ident_at(level, at - 1).is_some_and(|before| {
                        ["impl", "for", "struct", "enum", "union", "trait"]
                            .contains(&before.as_str())
                    }));
            entry && opened && !declared
        })
    })
}

/// Whether `tokens` reach one of `fields` as a field (`x.kind`, not `x.kind()`).
fn reaches_a_field(tokens: TokenStream, fields: &BTreeSet<String>) -> bool {
    levels(tokens).iter().any(|level| {
        (1..level.len()).any(|at| {
            let lone_dot = matches!(
                level.get(at - 1),
                Some(TokenTree::Punct(punct)) if punct.as_char() == '.' && punct.spacing() == Spacing::Alone
            ) && !(at >= 2 && punct_at(level, at - 2, '.'));
            let field = ident_at(level, at).is_some_and(|name| fields.contains(&name));
            let called = parenthesized_at(level, at + 1) || path_separator_at(level, at + 1);
            lone_dot && field && !called
        })
    })
}

/// Whether `body` holds an item: a keyword that opens one, a lifetime's name aside.
fn holds_an_item(body: TokenStream) -> bool {
    levels(body).iter().any(|level| {
        (0..level.len()).any(|at| {
            let keyword = matches!(
                level.get(at),
                Some(TokenTree::Ident(ident)) if ITEM_KEYWORDS.contains(&ident.to_string().as_str())
            );
            let lifetime = at > 0 && punct_at(level, at - 1, '\'');
            keyword && !lifetime
        })
    })
}

/// The new entry's fields, as `rows` declares them.
fn entry_fields(rows: &syn::File) -> BTreeSet<String> {
    every_item(&rows.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Struct(structure) if structure.ident == NEW_CHANGELOG_ENTRY => Some(structure),
            _ => None,
        })
        .flat_map(|structure| &structure.fields)
        .filter_map(|field| field.ident.as_ref().map(unraw))
        .collect()
}

/// What breaks the seal inside `rows`: a unit naming a gated constructor; one naming the
/// helper that isn't a gated or open constructor; one building or taking apart an entry by
/// hand that isn't on [`ENTRY_BUILDERS`]; one reaching an entry's field that isn't an
/// accessor; and a body holding an item.
fn rows_seal_findings(rows: &syn::File, gated: &BTreeSet<String>) -> Vec<String> {
    let names = EntryNames::of(rows);
    let fields = entry_fields(rows);
    let label_of = |name: &&str| format!("{NEW_CHANGELOG_ENTRY}::{name}");
    let helper_callers: BTreeSet<String> = GATED_CONSTRUCTORS
        .iter()
        .chain(&OPEN_CONSTRUCTORS)
        .map(label_of)
        .collect();
    let readers: BTreeSet<String> = ACCESSORS.iter().map(label_of).collect();
    let mut out = Vec::new();
    for unit in rows_units(rows, &names) {
        let label = &unit.label;
        for name in path_last_segments(unit.tokens.clone()) {
            if gated.contains(&name) {
                out.push(format!(
                    "{ROWS_FILE}: {label} names the gated constructor `{name}`"
                ));
            } else if name == ENTRY_HELPER && !helper_callers.contains(label) {
                out.push(format!("{ROWS_FILE}: {label} names `{ENTRY_HELPER}`"));
            }
        }
        let builds = builds_an_entry(unit.tokens.clone(), &names, unit.self_is_entry);
        if builds && !ENTRY_BUILDERS.contains(&label.as_str()) {
            out.push(format!("{ROWS_FILE}: {label} builds an entry by hand"));
        }
        if reaches_a_field(unit.tokens.clone(), &fields) && !readers.contains(label) {
            out.push(format!("{ROWS_FILE}: {label} reaches an entry's field"));
        }
        if unit.body.is_some_and(holds_an_item) {
            out.push(format!("{ROWS_FILE}: {label} holds an item in its body"));
        }
    }
    out
}

/// Each gated constructor `source`, the file `file`, names as a path's last segment (`T::c`,
/// `<T>::c`, `Self::c`, whatever `T` is), when the file may not name one.
fn gated_constructor_findings(file: &str, source: &str, gated: &BTreeSet<String>) -> Vec<String> {
    if GATED_CONSTRUCTOR_FILES.contains(&file) {
        return Vec::new();
    }
    let parsed = syn::parse_file(source).expect("the source parses");
    pieces(&parsed)
        .into_iter()
        .flat_map(|piece| {
            path_last_segments(piece.tokens)
                .into_iter()
                .filter(|name| gated.contains(name))
                .map(move |name| format!("{file}: `{name}` in {}", piece.context))
        })
        .collect()
}

/// The application's clippy bans on the gated constructors, and what breaks their tie to
/// `gated`: a gated constructor it doesn't ban, and one it bans that isn't gated, each by the
/// path `domain::elements::commission::NewChangelogEntry::<name>`.
fn constructor_ban_findings(settings: &str, gated: &BTreeSet<String>) -> Vec<String> {
    let prefix = format!("domain::elements::commission::{NEW_CHANGELOG_ENTRY}::");
    let banned: BTreeSet<String> = quoted_strings(settings)
        .into_iter()
        .filter(|text| is_path(text))
        .filter_map(|path| path.strip_prefix(&prefix).map(str::to_owned))
        .collect();
    let unbanned = gated
        .difference(&banned)
        .map(|name| format!("clippy.toml doesn't ban `{prefix}{name}`"));
    let ungated = banned
        .difference(gated)
        .map(|name| format!("clippy.toml bans `{prefix}{name}`, which isn't gated"));
    unbanned.chain(ungated).collect()
}

/// Each of `attributes` but the docs, as written with its whitespace dropped.
fn undocumented(attributes: &[syn::Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| !attribute.path().is_ident("doc"))
        .map(|attribute| squeezed(&attribute.to_token_stream().to_string()))
        .collect()
}

/// The attributes `rows` carries at any depth beyond docs and [`ROWS_ATTRIBUTES`], the entry
/// types' own aside ([`entry_type_findings`] reads those), in one finding.
fn rows_attribute_findings(rows: &syn::File) -> Vec<String> {
    let mut others = rows.clone();
    for item in &mut others.items {
        if let Item::Struct(structure) = item
            && ENTRY_TYPES.iter().any(|entry| structure.ident == entry)
        {
            structure.attrs.clear();
        }
    }
    let mut extra = Vec::new();
    for level in levels(others.to_token_stream()) {
        for at in 0..level.len() {
            if !punct_at(&level, at, '#') {
                continue;
            }
            let inner = punct_at(&level, at + 1, '!');
            let body_at = if inner { at + 2 } else { at + 1 };
            let Some(TokenTree::Group(body)) = level.get(body_at) else {
                continue;
            };
            if body.delimiter() != Delimiter::Bracket {
                continue;
            }
            let opening: Vec<TokenTree> = body.stream().into_iter().take(1).collect();
            let doc = ident_at(&opening, 0).as_deref() == Some("doc");
            let bang = if inner { "!" } else { "" };
            let written = squeezed(&format!("#{bang}{body}"));
            if !doc && !ROWS_ATTRIBUTES.contains(&written.as_str()) {
                extra.push(written);
            }
        }
    }
    if extra.is_empty() {
        return Vec::new();
    }
    let finding = format!("{ROWS_FILE}: carries {extra:?} beside docs and {ROWS_ATTRIBUTES:?}");
    vec![finding]
}

/// What breaks the entry types' declarations in `rows`: each is declared once, carries exactly
/// [`ENTRY_ATTRIBUTES`] beside its docs, and keeps every field private.
fn entry_type_findings(rows: &syn::File) -> Vec<String> {
    let mut out = Vec::new();
    for entry in ENTRY_TYPES {
        let declared: Vec<&syn::ItemStruct> = every_item(&rows.items)
            .into_iter()
            .filter_map(|item| match item {
                Item::Struct(structure) if structure.ident == entry => Some(structure),
                _ => None,
            })
            .collect();
        let [structure] = declared.as_slice() else {
            out.push(format!(
                "{ROWS_FILE}: `{entry}` is declared {} times, not once",
                declared.len()
            ));
            continue;
        };
        let attributes = undocumented(&structure.attrs);
        if attributes != ENTRY_ATTRIBUTES {
            out.push(format!(
                "{ROWS_FILE}: `{entry}` carries {attributes:?}, not exactly {ENTRY_ATTRIBUTES:?}"
            ));
        }
        let visible = structure
            .fields
            .iter()
            .filter(|field| !matches!(field.vis, Visibility::Inherited));
        for field in visible {
            out.push(format!(
                "{ROWS_FILE}: `{entry}` has a visible field: `{}`",
                field.to_token_stream()
            ));
        }
    }
    out
}

/// Each module `rows` declares but its unit tests (`#[cfg(test)] mod tests;`, beside its docs),
/// and each macro it calls in an item's place: a child module sees an entry's private fields,
/// and a macro could declare one.
fn rows_module_findings(rows: &syn::File) -> Vec<String> {
    every_item(&rows.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Mod(module) => {
                let unit_tests = module.ident == ROWS_TESTS
                    && module.content.is_none()
                    && matches!(module.vis, Visibility::Inherited)
                    && undocumented(&module.attrs) == ROWS_TESTS_ATTRIBUTES;
                (!unit_tests).then(|| format!("{ROWS_FILE}: declares {}", item_name(item)))
            }
            Item::Macro(_) => Some(format!(
                "{ROWS_FILE}: calls a macro in an item's place: {}",
                item_name(item)
            )),
            _ => None,
        })
        .collect()
}

/// Whether the `impl` at `at` in `level` opens an impl item, not an `impl Trait` type: it
/// starts its level, or follows a `;`, a block, an attribute, `unsafe` or `default`.
fn opens_an_impl_item(level: &[TokenTree], at: usize) -> bool {
    let Some(before) = at.checked_sub(1) else {
        return true;
    };
    let after_an_item = punct_at(level, before, ';')
        || matches!(
            &level[before],
            TokenTree::Group(group)
                if matches!(group.delimiter(), Delimiter::Brace | Delimiter::Bracket)
        );
    let qualified =
        ident_at(level, before).is_some_and(|word| word == "unsafe" || word == "default");
    after_an_item || qualified
}

/// The header (`impl` to its block) of each inherent impl in `tokens` naming one of `names`, at
/// any depth: a trait impl's `for`, unlike a `for<…>` bound, sets an impl aside.
fn inherent_impl_headers(tokens: TokenStream, names: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for level in levels(tokens) {
        for at in 0..level.len() {
            if ident_at(&level, at).as_deref() != Some("impl") || !opens_an_impl_item(&level, at) {
                continue;
            }
            let header: Vec<TokenTree> = level[at + 1..]
                .iter()
                .take_while(|tree| !is_brace_group(tree))
                .cloned()
                .collect();
            let for_a_trait = (0..header.len()).any(|index| {
                ident_at(&header, index).as_deref() == Some("for")
                    && !punct_at(&header, index + 1, '<')
            });
            let written: TokenStream = header.into_iter().collect();
            let named = identifiers(written.clone())
                .iter()
                .any(|ident| names.contains(ident));
            if named && !for_a_trait {
                out.push(format!("impl {written}"));
            }
        }
    }
    out
}

/// Each inherent impl of an entry type in `files` (each by its path from the crates folder,
/// with its source) outside the rows file, under any alias the files make, at any depth: in an
/// inline or child module, a function or const body, or a macro's input.
fn entry_impls_elsewhere(files: &[(String, String)]) -> Vec<String> {
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let names = token_aliases(&ENTRY_TYPES, &sources);
    files
        .iter()
        .filter(|(file, _)| file != ROWS_FILE)
        .flat_map(|(file, source)| {
            let parsed = syn::parse_file(source).expect("the source parses");
            inherent_impl_headers(parsed.to_token_stream(), &names)
                .into_iter()
                .map(move |header| {
                    format!("{file}: `{header}` is an inherent impl of an entry type")
                })
        })
        .collect()
}

/// Every seal finding in the rows file `source`, with the gated constructors it defines.
fn rows_findings(source: &str) -> (Vec<String>, BTreeSet<String>) {
    let rows = syn::parse_file(source).expect("the rows file parses");
    let gated = gated_constructors(&rows);
    let found = entry_type_findings(&rows)
        .into_iter()
        .chain(rows_attribute_findings(&rows))
        .chain(rows_module_findings(&rows))
        .chain(entry_surface_findings(&rows))
        .chain(rows_seal_findings(&rows, &gated))
        .collect();
    (found, gated)
}

#[test]
fn the_gated_constructors_are_named_only_in_the_rows_file_and_the_act_file() {
    let rows_source =
        std::fs::read_to_string(changelog_rows_file()).expect("the rows file is readable");
    let (in_rows, gated) = rows_findings(&rows_source);
    let everywhere: Vec<(String, String)> = workspace_production()
        .iter()
        .map(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            (relative(path, &crates()), source)
        })
        .collect();

    let found: Vec<String> = everywhere
        .iter()
        .flat_map(|(file, source)| gated_constructor_findings(file, source, &gated))
        .collect();
    let impls_elsewhere = entry_impls_elsewhere(&everywhere);

    assert_eq!(found, Vec::<String>::new());
    assert_eq!(in_rows, Vec::<String>::new());
    assert_eq!(impls_elsewhere, Vec::<String>::new());
    assert!(
        everywhere.iter().any(|(file, _)| file == ROWS_FILE),
        "the scan reads the rows file by its path"
    );
    let expected: BTreeSet<String> = GATED_CONSTRUCTORS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(gated, expected);
    let surface: BTreeSet<(&str, &str)> = ENTRY_SURFACE
        .iter()
        .map(|(owner, name, _)| (*owner, *name))
        .collect();
    let listed = ACCESSORS
        .iter()
        .chain(&OPEN_CONSTRUCTORS)
        .chain([&ENTRY_HELPER])
        .map(|name| (NEW_CHANGELOG_ENTRY, *name))
        .chain([(OPEN_CHANGELOG_ENTRY, OPEN_ENTRY_NOTE)]);
    for item in listed {
        assert!(surface.contains(&item), "{item:?} is pinned");
    }
    for file in GATED_CONSTRUCTOR_FILES {
        assert!(crates().join(file).is_file(), "{file} exists");
    }
    let act = application_src().join(ACT_FILE);
    let act_source = std::fs::read_to_string(&act).expect("the act file reads");
    let named_in_act =
        gated_constructor_findings("application/src/den/write.rs", &act_source, &gated);
    assert_eq!(
        named_in_act.len(),
        GATED_CONSTRUCTORS.len(),
        "the scan reads every naming: {named_in_act:?}"
    );
    let settings =
        std::fs::read_to_string(clippy_settings()).expect("application's clippy.toml is readable");
    assert_eq!(
        constructor_ban_findings(&settings, &gated),
        Vec::<String>::new()
    );
    let route = "api/src/routes/commissions/rename.rs";
    let refused = [
        (
            route,
            "fn forge(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::renamed(c, a, t) }",
        ),
        (
            route,
            "use domain::elements::commission::NewChangelogEntry as Entry;\nfn forge(c: CommissionId, m: &LateMark, t: DateTimeUtc) -> Entry { Entry::late(c, m, t) }",
        ),
        (
            route,
            "fn constructor() -> fn(CommissionId, UserId, DateTimeUtc) -> NewChangelogEntry { domain::elements::commission::NewChangelogEntry::created }",
        ),
        (
            route,
            "fn forge(c: CommissionId, a: UserId, s: &DeadlineSet, t: DateTimeUtc) -> NewChangelogEntry { <NewChangelogEntry>::deadline_set(c, a, s, t) }",
        ),
        (
            route,
            "type Entry = NewChangelogEntry;\nmacro_rules! forge { ($c:expr) => { Entry::delayed_set($c) }; }",
        ),
        (
            "application/src/commission/archive.rs",
            "fn forge(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::renamed(c, a, t) }",
        ),
        (
            "adapter-mem/src/fixture/commission.rs",
            "fn forge(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::created(c, a, t) }",
        ),
        (
            "workflow/src/elements/commission/fact.rs",
            "fn forge(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::renamed(c, a, t) }",
        ),
        (
            "workflow/src/elements/commission/deadline.rs",
            "impl LateMark { pub fn entry(&self, commission: CommissionId, at: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::late(commission, self, at) } }",
        ),
    ];
    for (file, source) in refused {
        let found_here = gated_constructor_findings(file, source, &gated);

        assert_eq!(found_here.len(), 1, "{file}: {source}: {found_here:?}");
    }
    let kept = [
        "fn note(c: CommissionId, a: UserId, t: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry::note(c, a, \"seen\".to_owned(), t) }",
        "fn kinds() -> bool { let created = 1; let late = ChangelogEntryKind::Late; entry.late == late && created == 1 }",
        "/// Built by `NewChangelogEntry::renamed`.\nfn doc() {}",
    ];
    for source in kept {
        assert_eq!(
            gated_constructor_findings(route, source, &gated),
            Vec::<String>::new(),
            "{source}"
        );
    }
    for file in GATED_CONSTRUCTOR_FILES {
        let allowed = gated_constructor_findings(file, refused[0].1, &gated);
        assert_eq!(allowed, Vec::<String>::new(), "{file}");
    }
    let in_entry_impl = |plant: &str| {
        let planted = rows_source.replacen(
            "impl NewChangelogEntry {\n",
            &format!("impl NewChangelogEntry {{\n{plant}\n"),
            1,
        );
        assert_ne!(planted, rows_source, "the plant lands in the entry's impl");
        planted
    };
    let after_the_entry = |plant: &str| format!("{rows_source}\n{plant}\n");
    let accessor = "pub fn kind(&self) -> ChangelogEntryKind {\n        self.kind\n    }";
    let re_signed = rows_source.replacen(
        accessor,
        "pub fn kind(&self) -> Option<NewChangelogEntry> {\n        None\n    }",
        1,
    );
    assert_ne!(re_signed, rows_source, "the accessor is re-signed");
    let rewritten = |from: &str, to: &str| {
        let planted = rows_source.replacen(from, to, 1);
        assert_ne!(planted, rows_source, "the rows file holds `{from}`");
        planted
    };
    let entry_derive = "#[derive(Debug)]\npub struct NewChangelogEntry {";
    let open_entry = "#[derive(Debug)]\npub struct OpenChangelogEntry(NewChangelogEntry);";
    let tests_module = "#[cfg(test)]\nmod tests;\n";
    let rows_refused: [(String, &[&str]); 23] = [
        (
            rewritten(
                entry_derive,
                "#[derive(Debug, serde::Deserialize)]\npub struct NewChangelogEntry {",
            ),
            &[],
        ),
        (
            rewritten(
                entry_derive,
                "#[derive(Debug, derive_more::From)]\npub struct NewChangelogEntry {",
            ),
            &[],
        ),
        (
            rewritten(tests_module, "#[cfg(test)]\nmod tests;\nmod system;\n"),
            &[],
        ),
        (
            rewritten(
                open_entry,
                "#[derive(Debug, Clone)]\npub struct OpenChangelogEntry(NewChangelogEntry);",
            ),
            &[],
        ),
        (
            rewritten(
                entry_derive,
                "#[derive(Debug)]\n#[cfg_attr(test, derive(Default))]\npub struct NewChangelogEntry {",
            ),
            &[],
        ),
        (
            rewritten(
                "    kind: ChangelogEntryKind,\n    actor_id",
                "    pub kind: ChangelogEntryKind,\n    actor_id",
            ),
            &[],
        ),
        (
            rewritten(
                open_entry,
                "#[derive(Debug)]\npub struct OpenChangelogEntry(pub NewChangelogEntry);",
            ),
            &[],
        ),
        (after_the_entry("mod system {\n    use super::*;\n}"), &[]),
        (rewritten(tests_module, "mod tests;\n"), &[]),
        (after_the_entry("declare_system!(system);"), &[]),
        (
            after_the_entry(
                "#[derive(Debug, serde::Deserialize)]\n#[serde(remote = \"NewChangelogEntry\")]\npub struct Mirror { commission_id: CommissionId, kind: ChangelogEntryKind, actor_id: Option<UserId>, payload: Value, note: Option<String>, created_at: DateTimeUtc }",
            ),
            &[],
        ),
        (
            rewritten(
                "impl NewChangelogEntry {\n",
                "#[forge::constructors]\nimpl NewChangelogEntry {\n",
            ),
            &[],
        ),
        (
            in_entry_impl(
                "pub fn lifecycle_moved_if(commission: CommissionId, actor: UserId, moved: Option<&LifecycleMove>, at: DateTimeUtc) -> Option<Self> { moved.map(|moved| Self::lifecycle_moved(commission, actor, moved, at)) }",
            ),
            &["lifecycle_moved_if"],
        ),
        (
            in_entry_impl(
                "pub(crate) fn late_by(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> Self { Self::by(commission, ChangelogEntryKind::Late, actor, json!({}), at) }",
            ),
            &["late_by"],
        ),
        (
            after_the_entry(
                "pub fn forged_late(commission: CommissionId, at: DateTimeUtc) -> NewChangelogEntry { NewChangelogEntry { commission_id: commission, kind: ChangelogEntryKind::Late, actor_id: None, payload: json!({}), note: None, created_at: at } }",
            ),
            &[],
        ),
        (
            after_the_entry(
                "impl From<&LateMark> for NewChangelogEntry { fn from(mark: &LateMark) -> Self { Self::late(CommissionId::from(Uuid::nil()), mark, DateTimeUtc::default()) } }",
            ),
            &[],
        ),
        (
            after_the_entry(
                "pub fn relabeled(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> NewChangelogEntry { let mut entry = NewChangelogEntry::note(commission, actor, String::new(), at); entry.kind = ChangelogEntryKind::Late; entry }",
            ),
            &[],
        ),
        (
            after_the_entry(
                "type Entry = NewChangelogEntry;\npub fn forged(commission: CommissionId, at: DateTimeUtc) -> Entry { Entry { commission_id: commission, kind: ChangelogEntryKind::Late, actor_id: None, payload: Value::Null, note: None, created_at: at } }",
            ),
            &[],
        ),
        (
            after_the_entry(
                "pub fn opened(entry: NewChangelogEntry) -> OpenChangelogEntry { OpenChangelogEntry(entry) }",
            ),
            &[],
        ),
        (
            after_the_entry(
                "const _: () = { impl NewChangelogEntry { pub fn forged(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> Self { Self::note(commission, actor, String::new(), at) } } };",
            ),
            &[],
        ),
        (
            after_the_entry("forge! { impl NewChangelogEntry { pub fn forged() {} } }"),
            &[],
        ),
        (
            in_entry_impl(
                "fn forged(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> Self { Self::note(commission, actor, String::new(), at) }",
            ),
            &[],
        ),
        (re_signed, &[]),
    ];
    for (planted, newly_gated) in rows_refused {
        let (found_here, planted_gated) = rows_findings(&planted);

        assert_eq!(found_here.len(), 1, "{planted}: {found_here:?}");
        let added: Vec<&str> = planted_gated
            .difference(&expected)
            .map(String::as_str)
            .collect();
        assert_eq!(added, newly_gated, "{found_here:?}");
    }
    let child = "workflow/src/elements/commission/changelog/rows/system.rs";
    let herons_child = r#"//! The system's entries, split out of the rows file.

use serde_json::Value;

use super::{ChangelogEntryKind, NewChangelogEntry};
use crate::elements::commission::CommissionId;
use kernel::datetime::DateTimeUtc;

impl NewChangelogEntry {
    /// A system entry of `kind` with `payload` and no actor.
    pub fn system(
        commission: CommissionId,
        kind: ChangelogEntryKind,
        payload: Value,
        at: DateTimeUtc,
    ) -> Self {
        Self {
            commission_id: commission,
            kind,
            actor_id: None,
            payload,
            note: None,
            created_at: at,
        }
    }
}
"#;
    let fact = "workflow/src/elements/commission/fact.rs";
    let impls_refused = [
        (child, herons_child),
        (
            child,
            "use super::NewChangelogEntry as Entry;\nimpl Entry {\n    pub fn forged() {}\n}",
        ),
        (
            child,
            "type Entry = super::OpenChangelogEntry;\nimpl Entry {}",
        ),
        (
            "workflow/src/elements/commission/deadline.rs",
            "fn hide() {\n    impl NewChangelogEntry {\n        pub fn forged() {}\n    }\n}",
        ),
        (
            fact,
            "const _: () = {\n    impl crate::elements::commission::NewChangelogEntry {}\n};",
        ),
        (
            fact,
            "#[doc = \"bound\"]\nimpl<T> NewChangelogEntry where T: for<'a> Fn(&'a str) {}",
        ),
    ];
    for (file, source) in impls_refused {
        let planted = [(file.to_owned(), source.to_owned())];

        let found_here = entry_impls_elsewhere(&planted);

        assert_eq!(found_here.len(), 1, "{file}: {source}: {found_here:?}");
    }
    let impls_kept = [
        (
            fact,
            "impl std::fmt::Display for NewChangelogEntry {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }\n}",
        ),
        (
            fact,
            "pub fn entries() -> impl Iterator<Item = NewChangelogEntry> {\n    std::iter::empty()\n}",
        ),
        (
            ROWS_FILE,
            "impl NewChangelogEntry {\n    pub fn forged() {}\n}",
        ),
    ];
    for (file, source) in impls_kept {
        let planted = [(file.to_owned(), source.to_owned())];

        let found_here = entry_impls_elsewhere(&planted);

        assert_eq!(found_here, Vec::<String>::new(), "{file}: {source}");
    }
    let closing = settings.rfind(']').expect("clippy.toml's list closes");
    let created_line = settings
        .lines()
        .find(|line| line.contains("NewChangelogEntry::created\""))
        .expect("clippy.toml bans created on its own line");
    let planted_settings = [
        settings.replacen(created_line, "", 1),
        format!(
            "{}{{ path = \"domain::elements::commission::NewChangelogEntry::note\", reason = \"no\" }},\n{}",
            &settings[..closing],
            &settings[closing..]
        ),
    ];
    for planted in planted_settings {
        assert_eq!(
            constructor_ban_findings(&planted, &gated).len(),
            1,
            "{planted}"
        );
    }
}

// Each commission write statement and each in-memory commission change, pinned to its method.

/// What each method of the PostgreSQL write view reaches: each generated commission write
/// statement (`statement f`), write helper (`helper h`) and write-view method (`method m`) it
/// names, by method. A new statement, or one named in a second method, fails until it is here.
const PG_VIEW_REACHES: [(&str, &str); 27] = [
    ("add_element", "helper insert_element"),
    ("add_file", "statement add_file"),
    ("add_markup", "statement add_markup"),
    ("clear_dispute", "statement clear_dispute"),
    ("create", "statement add_participant"),
    ("create", "statement create_commission"),
    ("create", "statement create_tab"),
    ("create_seat_invitation", "statement create_seat_invitation"),
    ("declare_seat", "helper insert_element"),
    ("declare_seat", "statement declare_seat_satellite"),
    ("declare_slots", "helper insert_element"),
    ("declare_slots", "statement declare_slot_satellite"),
    ("delete", "statement delete"),
    ("grant_view", "statement grant_view"),
    ("insert_element", "statement add_element"),
    ("remove_element", "statement remove_element_delete"),
    ("remove_element", "statement remove_element_renumber"),
    ("revoke_seat_invitation", "statement revoke_seat_invitation"),
    ("revoke_view", "statement revoke_view"),
    ("set_archived", "statement set_archived"),
    ("set_deadline", "statement set_deadline"),
    ("set_deadline_status", "statement set_deadline_status"),
    ("set_direction_status", "statement set_direction_status"),
    ("set_dispute", "statement set_dispute"),
    ("set_lifecycle", "statement set_lifecycle"),
    ("set_linked_channel", "statement set_linked_channel"),
    ("set_maturity", "statement set_maturity"),
];

/// What each in-memory piece that may change a commission reaches: each change to the
/// commission map (`map m`, `map handed on`), each write to a stored commission's field
/// (`row f`), each call of the insert path (`calls insert_commission`) and each write-view
/// method called on the view (`method m`), by piece.
const MEM_VIEW_CHANGES: [(&str, &str); 27] = [
    (
        "commission.rs: MemBackend::seed_bound_commission",
        "calls insert_commission",
    ),
    (
        "commission.rs: MemBackend::seed_lifecycle_step",
        "map get_mut",
    ),
    (
        "commission.rs: MemBackend::seed_lifecycle_step",
        "row lifecycle_step",
    ),
    (
        "commission.rs: MemCommissionWrites::clear_dispute",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::clear_dispute",
        "row dispute",
    ),
    (
        "commission.rs: MemCommissionWrites::clear_dispute",
        "row lifecycle_step",
    ),
    (
        "commission.rs: MemCommissionWrites::create",
        "calls insert_commission",
    ),
    ("commission.rs: MemCommissionWrites::delete", "map remove"),
    (
        "commission.rs: MemCommissionWrites::set_archived",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_archived",
        "row archived_at",
    ),
    (
        "commission.rs: MemCommissionWrites::set_deadline",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_deadline",
        "row deadline",
    ),
    (
        "commission.rs: MemCommissionWrites::set_deadline_status",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_deadline_status",
        "row delayed_flag",
    ),
    (
        "commission.rs: MemCommissionWrites::set_direction_status",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_direction_status",
        "row direction_status",
    ),
    (
        "commission.rs: MemCommissionWrites::set_dispute",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_dispute",
        "row dispute",
    ),
    (
        "commission.rs: MemCommissionWrites::set_dispute",
        "row lifecycle_step",
    ),
    (
        "commission.rs: MemCommissionWrites::set_lifecycle",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_lifecycle",
        "row lifecycle_step",
    ),
    (
        "commission.rs: MemCommissionWrites::set_linked_channel",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_linked_channel",
        "row linked_channel",
    ),
    (
        "commission.rs: MemCommissionWrites::set_maturity",
        "map get_mut",
    ),
    (
        "commission.rs: MemCommissionWrites::set_maturity",
        "row maturity",
    ),
    ("commission.rs: insert_commission", "map insert"),
    ("lib.rs: MemBackend::merge", "map handed on"),
];

/// The methods that change an `Option` field (or any field) of a stored commission in place.
const FIELD_MUTATORS: [&str; 18] = [
    "take",
    "take_if",
    "replace",
    "insert",
    "get_or_insert",
    "get_or_insert_with",
    "as_mut",
    "as_deref_mut",
    "iter_mut",
    "push",
    "clear",
    "extend",
    "retain",
    "truncate",
    "drain",
    "append",
    "remove",
    "swap",
];

/// The table's pairs, by name.
fn reach_table(table: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    table
        .iter()
        .map(|(context, reach)| ((*context).to_owned(), (*reach).to_owned()))
        .collect()
}

/// Each write-view method `level` calls on a receiver or a path whose last segment before it
/// is no generated query module of `modules`: `self.m(…)`, `Self::m`, `CommissionWrites::m`.
fn write_method_calls(
    level: &[TokenTree],
    modules: &BTreeSet<String>,
    write_methods: &BTreeSet<String>,
) -> Vec<String> {
    (0..level.len())
        .filter_map(|at| {
            let name = ident_at(level, at).filter(|name| write_methods.contains(name))?;
            let after_dot = at >= 1 && punct_at(level, at - 1, '.');
            let after_path = at >= 2 && path_separator_at(level, at - 2);
            let through_a_module = at >= 3
                && ident_at(level, at - 3).is_some_and(|segment| modules.contains(&segment));
            let called = after_dot || (after_path && !through_a_module);
            called.then_some(name)
        })
        .collect()
}

/// Every reach of each method of the PostgreSQL write view in the commission file among
/// `files`, as [`PG_VIEW_REACHES`] writes it.
fn pg_view_reaches(
    files: &[(String, String)],
    write_fns: &BTreeSet<String>,
    write_methods: &BTreeSet<String>,
) -> BTreeSet<(String, String)> {
    let Some((_, source)) = files.iter().find(|(file, _)| file == COMMISSION_FILE) else {
        return BTreeSet::new();
    };
    let parsed = syn::parse_file(source).expect("the commission file parses");
    let file_pieces = pieces(&parsed);
    let file_levels: Vec<Vec<TokenTree>> = file_pieces
        .iter()
        .flat_map(|piece| levels(piece.tokens.clone()))
        .collect();
    let modules = query_module_names(&file_levels);
    let helpers = pg_write_helpers(&file_pieces, &modules, write_fns);
    let mut out = BTreeSet::new();
    let methods = file_pieces
        .iter()
        .filter(|piece| in_the_pg_write_view(piece) && piece.context.contains("::"));
    for piece in methods {
        let method = method_of(piece).to_owned();
        for level in levels(piece.tokens.clone()) {
            let statements = write_fn_namings(&level, &modules, write_fns)
                .into_iter()
                .map(|name| format!("statement {name}"));
            let helper_calls = calls_into(&level, &helpers)
                .into_iter()
                .map(|name| format!("helper {name}"));
            let method_calls = write_method_calls(&level, &modules, write_methods)
                .into_iter()
                .map(|name| format!("method {name}"));
            for reach in statements.chain(helper_calls).chain(method_calls) {
                out.insert((method.clone(), reach));
            }
        }
    }
    out
}

/// Whether an assignment operator starts at `at`: `=`, a compound one (`+=`, `|=`, …) or a
/// shift's (`<<=`, `>>=`); never a comparison.
fn assignment_at(level: &[TokenTree], at: usize) -> bool {
    let alone_equals = |index: usize| {
        matches!(
            level.get(index),
            Some(TokenTree::Punct(punct)) if punct.as_char() == '=' && punct.spacing() == Spacing::Alone
        )
    };
    let compound = matches!(
        level.get(at),
        Some(TokenTree::Punct(punct)) if "+-*/%^&|".contains(punct.as_char()) && punct.spacing() == Spacing::Joint
    );
    let shift = (punct_at(level, at, '<') && punct_at(level, at + 1, '<'))
        || (punct_at(level, at, '>') && punct_at(level, at + 1, '>'));
    alone_equals(at) || (compound && alone_equals(at + 1)) || (shift && alone_equals(at + 2))
}

/// The first token of the field path ending at `at`: `a.b.0.name`, a borrow before it aside.
fn path_start(level: &[TokenTree], at: usize) -> usize {
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
    start
}

/// Each field of `fields` `level` changes on a value: assigned (`x.f = …`, `x.f += …`),
/// borrowed mutably (`&mut x.f`, `&raw mut x.f`), or passed to one of [`FIELD_MUTATORS`].
fn field_changes(level: &[TokenTree], fields: &BTreeSet<String>) -> Vec<String> {
    (0..level.len())
        .filter_map(|at| {
            let name = ident_at(level, at).filter(|name| fields.contains(name))?;
            let on_a_value = at >= 1 && punct_at(level, at - 1, '.');
            let start = path_start(level, at);
            let borrowed_mut = (start >= 2
                && punct_at(level, start - 2, '&')
                && ident_at(level, start - 1).as_deref() == Some("mut"))
                || (start >= 3
                    && punct_at(level, start - 3, '&')
                    && ident_at(level, start - 2).as_deref() == Some("raw")
                    && ident_at(level, start - 1).as_deref() == Some("mut"));
            let mutated = punct_at(level, at + 1, '.')
                && ident_at(level, at + 2)
                    .is_some_and(|method| FIELD_MUTATORS.contains(&method.as_str()));
            let changed = assignment_at(level, at + 1) || borrowed_mut || mutated;
            (on_a_value && changed).then_some(name)
        })
        .collect()
}

/// The fields of the in-memory commission row, read from its definition in `files`.
fn stored_commission_fields(files: &[(String, String)]) -> BTreeSet<String> {
    let (_, source) = files
        .iter()
        .find(|(file, _)| file == COMMISSION_FILE)
        .expect("the in-memory adapter's commission file");
    let parsed = syn::parse_file(source).expect("the commission file parses");
    every_item(&parsed.items)
        .into_iter()
        .filter_map(|item| match item {
            Item::Struct(structure) if structure.ident == STORED_COMMISSION => Some(structure),
            _ => None,
        })
        .flat_map(|structure| structure.fields.iter())
        .filter_map(|field| field.ident.as_ref().map(unraw))
        .collect()
}

/// Every change each in-memory piece makes to a commission, as [`MEM_VIEW_CHANGES`] writes it,
/// in the adapter's sources `files`: the commission map changed or handed on in each piece
/// that may change it, a stored commission's field written and the insert path called in any
/// piece, and a write-view method called in any piece of the view.
fn mem_view_changes(files: &[(String, String)]) -> BTreeSet<(String, String)> {
    let fields = stored_commission_fields(files);
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let views = token_aliases(&[MEM_WRITE_VIEW], &sources);
    let write_methods = commission_write_methods();
    let mut out = BTreeSet::new();
    for (file, source) in files {
        let parsed = syn::parse_file(source).expect("the source parses");
        for piece in pieces(&parsed) {
            let context = piece.context.clone();
            let mut found: Vec<String> = Vec::new();
            if changes_the_mem_map(file, &piece) {
                for (reach, _) in piece_map_uses(&piece) {
                    match reach {
                        MapUse::Changed(method) => found.push(format!("map {method}")),
                        MapUse::Escaped => found.push("map handed on".to_owned()),
                        MapUse::Read | MapUse::Bound(_) => {}
                    }
                }
            }
            let in_view = piece
                .impl_of
                .as_ref()
                .is_some_and(|name| views.contains(name));
            for level in levels(piece.tokens.clone()) {
                found.extend(
                    field_changes(&level, &fields)
                        .into_iter()
                        .map(|field| format!("row {field}")),
                );
                let inserts = (0..level.len()).filter(|at| {
                    ident_at(&level, *at).as_deref() == Some(INSERT_COMMISSION)
                        && !defined_at(&level, *at)
                });
                found.extend(inserts.map(|_| format!("calls {INSERT_COMMISSION}")));
                if in_view {
                    let calls = write_method_calls(&level, &BTreeSet::new(), &write_methods);
                    found.extend(calls.into_iter().map(|name| format!("method {name}")));
                }
            }
            for reach in found {
                out.insert((format!("{file}: {context}"), reach));
            }
        }
    }
    out
}

#[test]
fn each_commission_write_statement_and_change_is_named_only_in_its_method() {
    let pg_files = pg_sources();
    let write_fns = pg_commission_write_fns();
    let write_methods = commission_write_methods();
    let mem_files = mem_sources();

    let pg_reaches = pg_view_reaches(&pg_files, &write_fns, &write_methods);
    let mem_changes = mem_view_changes(&mem_files);

    assert_eq!(pg_reaches, reach_table(&PG_VIEW_REACHES));
    assert_eq!(mem_changes, reach_table(&MEM_VIEW_CHANGES));
    let pinned: BTreeSet<String> = PG_VIEW_REACHES
        .iter()
        .filter_map(|(_, reach)| reach.strip_prefix("statement "))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        pinned, write_fns,
        "each generated write statement has its row"
    );
    let fields = stored_commission_fields(&mem_files);
    assert!(
        fields.contains("archived_at") && fields.contains("lifecycle_step"),
        "{fields:?}"
    );
    let pg_markup =
        "    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {";
    let pg_archive = "            sql::set_archived(&mut *self.conn, uuid::Uuid::from(*id), archived_at).await?;";
    let pg_refused = [
        (
            pg_markup,
            "sql::set_archived(&mut *self.conn, uuid::Uuid::from(markup.commission_id), None).await?;",
            ("add_markup", "statement set_archived"),
        ),
        (
            pg_markup,
            "self.set_archived(&markup.commission_id, None).await?;",
            ("add_markup", "method set_archived"),
        ),
        (
            pg_markup,
            "CommissionWrites::delete(self, &markup.commission_id).await?;",
            ("add_markup", "method delete"),
        ),
        (
            pg_markup,
            "use sql::{{set_maturity as rate}}; rate(&mut *self.conn, uuid::Uuid::nil(), None, None).await?;",
            ("add_markup", "statement set_maturity"),
        ),
        (
            pg_markup,
            "self.insert_element(&element).await?;",
            ("add_markup", "helper insert_element"),
        ),
        (
            pg_archive,
            "sql::set_linked_channel(&mut *self.conn, uuid::Uuid::from(*id), None, None).await?;",
            ("set_archived", "statement set_linked_channel"),
        ),
    ];
    for (anchor, addition, expected) in pg_refused {
        let planted = planted_after(&pg_files, COMMISSION_FILE, anchor, addition);

        let found = pg_view_reaches(&planted, &write_fns, &write_methods);

        let (method, reach) = expected;
        let extra = (method.to_owned(), reach.to_owned());
        assert!(found.contains(&extra), "{addition}: {found:?}");
        assert_ne!(found, reach_table(&PG_VIEW_REACHES), "{addition}");
    }
    let mem_markup =
        "    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {";
    let mem_maturity = "        stored.maturity = Some(change.after());";
    let lock = ".lock().expect(\"MemBackend commissions mutex poisoned\")";
    let markup_context = "commission.rs: MemCommissionWrites::add_markup";
    let mem_refused = [
        (
            mem_markup,
            format!(
                "if let Some(stored) = self.0.commissions{lock}.get_mut(&markup.commission_id) {{ stored.archived_at = None; }}"
            ),
            (markup_context, "row archived_at"),
        ),
        (
            mem_markup,
            format!(
                "let mut commissions = self.0.commissions{lock}; let stored = commissions.get_mut(&markup.commission_id).expect(\"a row\"); let _ = std::mem::take(&mut stored.deadline);"
            ),
            (markup_context, "row deadline"),
        ),
        (
            mem_markup,
            format!(
                "let mut commissions = self.0.commissions{lock}; if let Some(stored) = commissions.get_mut(&markup.commission_id) {{ stored.direction_status.take(); }}"
            ),
            (markup_context, "map get_mut"),
        ),
        (
            mem_markup,
            "self.set_archived(&markup.commission_id, None).await?;".to_owned(),
            (markup_context, "method set_archived"),
        ),
        (
            mem_maturity,
            "        stored.archived_at = None;".to_owned(),
            (
                "commission.rs: MemCommissionWrites::set_maturity",
                "row archived_at",
            ),
        ),
        (
            "impl StoredCommission {",
            "    fn unarchive(stored: &mut StoredCommission) { stored.archived_at = None; }"
                .to_owned(),
            (
                "commission.rs: StoredCommission::unarchive",
                "row archived_at",
            ),
        ),
    ];
    for (anchor, addition, expected) in &mem_refused {
        let planted = planted_after(&mem_files, COMMISSION_FILE, anchor, addition);

        let found = mem_view_changes(&planted);

        let (context, reach) = expected;
        let extra = ((*context).to_owned(), (*reach).to_owned());
        assert!(found.contains(&extra), "{addition}: {found:?}");
        assert_ne!(found, reach_table(&MEM_VIEW_CHANGES), "{addition}");
    }
    let mem_store = "impl CommissionStore for MemCommissionStore {";
    let mem_kept = format!(
        "    async fn probe(&self, id: &CommissionId) -> anyhow::Result<bool> {{ let commissions = self.0.commissions{lock}; Ok(commissions.get(id).is_some_and(|stored| stored.archived_at.is_none() && stored.deadline.map(|at| at > chrono::Utc::now()).is_some() && stored.maturity != None)) }}"
    );
    let kept = planted_after(&mem_files, COMMISSION_FILE, mem_store, &mem_kept);
    assert_eq!(mem_view_changes(&kept), reach_table(&MEM_VIEW_CHANGES));
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
