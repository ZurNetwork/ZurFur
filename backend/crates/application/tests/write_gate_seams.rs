//! The den's write gate is the only way to write a commission's node or fields. These scans
//! hold its seams: the write capability's surface, its one issuer across the workspace, the
//! write host and the gate named once each, the write use case kept inside the crate, nothing
//! of it (and no raw SQL) in a driver, and the commission field writes not yet gated, frozen.

mod seams;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use seams::{
    NODE_PORT, crates, declares_under_cfg_test, drivers, drivers_of, fixture_tree, identifiers,
    item_name, members_of, metadata, walk, with_aliases, workspace_production, workspace_sources,
};
use syn::{ImplItem, Item, TraitItem, Type, Visibility, ext::IdentExt};

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

/// The commission repo's field writes the gate doesn't hold yet, by name.
const FIELD_WRITES: [&str; 7] = [
    "set_title",
    "set_archived",
    "set_maturity",
    "set_direction_status",
    "set_deadline",
    "set_deadline_status",
    "declare_slots",
];

/// The commission repo's writes counted only on a commission repo: their names are common.
const REPO_WRITES: [&str; 2] = ["create", "delete"];

/// The SQL words a table name follows.
const SQL_KEYWORDS: [&str; 10] = [
    "FROM", "INTO", "UPDATE", "JOIN", "TABLE", "TRUNCATE", "COPY", "LOCK", "USING", "MERGE",
];

/// The den's node table.
const NODE_TABLE: &str = "node";

/// The ungated commission field writes in `application`, by file, function and method: each
/// leaves this list when the den's write gate takes it over, until the list is empty.
const FROZEN_FIELD_WRITES: [(&str, &str, &str); 11] = [
    (
        "commission/archive.rs",
        "Commissions::archive",
        "set_archived",
    ),
    ("commission/create.rs", "Commissions::create", "create"),
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
    ("commission/rename.rs", "Commissions::rename", "set_title"),
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
/// trait, `module::` before it in an inline module), or the item that holds no function.
struct Piece {
    context: String,
    impl_of: Option<String>,
    tokens: TokenStream,
}

fn pieces_in(items: &[Item], prefix: &str, out: &mut Vec<Piece>) {
    let piece = |context: String, impl_of: Option<String>, tokens: TokenStream| Piece {
        context,
        impl_of,
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
                out.push(piece(
                    format!("{prefix}impl {self_name}"),
                    Some(self_name.clone()),
                    header,
                ));
                for impl_item in &implementation.items {
                    let context = match impl_item {
                        ImplItem::Fn(method) => {
                            format!("{prefix}{self_name}::{}", method.sig.ident)
                        }
                        _ => format!("{prefix}impl {self_name}"),
                    };
                    out.push(piece(
                        context,
                        Some(self_name.clone()),
                        impl_item.to_token_stream(),
                    ));
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

/// What a driver may not name: the node port, the issuer, the capability, and raw SQL.
fn driver_forbidden(sources: &[String]) -> BTreeSet<String> {
    let seeds: Vec<&str> = NODE_PORT
        .iter()
        .copied()
        .chain([GATE_ISSUED, WRITE_CAPABILITY])
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

/// Each string literal in `tokens` that names the node table or a `.sql` file.
fn sql_findings(tokens: TokenStream) -> Vec<String> {
    string_literals(tokens)
        .into_iter()
        .filter(|text| names_the_node_table(text) || text.to_ascii_lowercase().ends_with(".sql"))
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
    let depends_on_composition = |package: &&serde_json::Value| {
        let dependencies = package["dependencies"]
            .as_array()
            .expect("a dependency list");
        dependencies.iter().any(|dependency| {
            dependency["name"] == seams::COMPOSITION && dependency["kind"].is_null()
        })
    };
    let expected: BTreeSet<String> = packages
        .iter()
        .filter(depends_on_composition)
        .filter_map(|package| package["name"].as_str())
        .filter(|name| *name != seams::TEST_SUPPORT)
        .chain([seams::COMPOSITION])
        .map(str::to_owned)
        .collect();
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
        ] }
    ] });
    let synthetic_drivers: Vec<String> = drivers_of(&members_of(&synthetic))
        .into_iter()
        .map(|member| member.name)
        .collect();
    assert_eq!(synthetic_drivers, ["composition", "worker", "builder"]);
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
        "fn setter() { let set = CommissionRepo::set_title; }",
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
        "trait Repo { async fn set_title(&mut self, id: &CommissionId, title: &Title); }",
    ];
    for source in not_field_writes {
        let file = syn::parse_file(source).expect("the planted source parses");

        let writes = field_writes("commission/other.rs", &file);

        assert_eq!(writes, Vec::<(String, String, String)>::new(), "{source}");
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
