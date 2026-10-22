//! The den's write gate is the only way to write a commission's node or fields. These scans
//! hold its seams: the write capability's surface, its one issuer across the workspace and the
//! issuer's callers in the den, the lent view's surface and its latch, nothing else in the gate
//! file reaching the lent view's fields, no compiled source the scans can't read, the write
//! host and the gate named once each, the write use case kept inside the crate, nothing of it
//! (and no raw SQL or in-memory seeder) in a driver, no lint silencer in the den, in
//! commission's acts (derived: every file implementing the gate's handler) or on the modules
//! above them, every method of the commission write view taking a capability or named, and
//! the commission field writes not yet gated, frozen.

mod seams;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use seams::{
    NODE_PORT, crates, declares_under_cfg_test, drivers, drivers_of, fixture_tree, identifiers,
    item_name, members_of, metadata, under_cfg_test, walk, with_aliases, workspace_production,
    workspace_sources,
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

/// The commission repo's field writes the gate doesn't hold yet, by name. Each leaves when its
/// method takes a write capability; the list is empty by M3.
const FIELD_WRITES: [&str; 6] = [
    "set_archived",
    "set_maturity",
    "set_direction_status",
    "set_deadline",
    "set_deadline_status",
    "declare_slots",
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

// No lint silencer in the den, in commission's acts or on the modules above them.

/// The lints nothing in the den's production files or commission's act files may silence:
/// an item no real act reaches is removed, never silenced. `warnings` silences them all.
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

/// The gate's handler trait: a file implementing it holds one of the den's acts.
const HANDLER: &str = "Handler";

/// Commission's files guarded by name though they implement no handler: the act enum's file
/// and commission's Den arm, relative to the use-case layer's sources.
const COMMISSION_ACT_FLOOR: [&str; 2] = ["commission/act.rs", "commission/den.rs"];

/// Whether `tokens`, at any depth, hold one of `handlers` followed by `for`, through any generic
/// arguments: an impl of the gate's handler, under a path or in a macro body alike.
fn implements_a_handler(tokens: TokenStream, handlers: &BTreeSet<String>) -> bool {
    levels(tokens).iter().any(|level| {
        (0..level.len()).any(|at| {
            let Some(ident) = ident_at(level, at) else {
                return false;
            };
            let after = if punct_at(level, at + 1, '<') {
                past_generics(level, at + 1)
            } else {
                at + 1
            };
            handlers.contains(&ident) && ident_at(level, after).as_deref() == Some("for")
        })
    })
}

/// Every production file under `src` that implements the gate's handler, relative to `src`:
/// [`HANDLER`], or a name the sources bind to it, followed by `for`.
fn handler_files(src: &Path) -> BTreeSet<String> {
    let sources: Vec<(String, String)> = walk(src)
        .production
        .iter()
        .map(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            (relative(path, src), source)
        })
        .collect();
    let texts: Vec<String> = sources.iter().map(|(_, source)| source.clone()).collect();
    let handlers = with_aliases(&[HANDLER], &texts);
    sources
        .into_iter()
        .filter(|(_, source)| {
            let tokens: TokenStream = source.parse().expect("the source lexes");
            implements_a_handler(tokens, &handlers)
        })
        .map(|(file, _)| file)
        .collect()
}

/// Commission's act files under `src`: every file implementing the gate's handler, and
/// [`COMMISSION_ACT_FLOOR`].
fn act_files(src: &Path) -> BTreeSet<String> {
    let mut files = handler_files(src);
    files.extend(COMMISSION_ACT_FLOOR.map(str::to_owned));
    files
}

/// Each silencer in the files `files` names, relative to `src`.
fn silencer_offenders(src: &Path, files: &[&str]) -> Vec<String> {
    files
        .iter()
        .flat_map(|file| {
            let source = std::fs::read_to_string(src.join(file)).expect("a readable source");
            silencer_findings(file, &source)
        })
        .collect()
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

/// The files above the `guarded` ones (paths relative to the use-case layer's sources) that
/// aren't guarded themselves, each with the modules it declares on the way down to a guarded
/// file: `lib.rs` declares a top module, `a.rs` declares `a/b.rs`'s `b`.
fn ancestor_modules(guarded: &[&str]) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in guarded {
        let modules: Vec<&str> = file.trim_end_matches(".rs").split('/').collect();
        for (depth, module) in modules.iter().enumerate() {
            let parent = match depth {
                0 => "lib.rs".to_owned(),
                _ => format!("{}.rs", modules[..depth].join("/")),
            };
            if !guarded.contains(&parent.as_str()) {
                out.entry(parent).or_default().insert((*module).to_owned());
            }
        }
    }
    out
}

/// Each silencer in the ancestor module file `file` that reaches a guarded file: an inner one,
/// which silences the whole file and its modules, or an outer one on the `mod` item declaring
/// a module in `declared`.
fn ancestor_silencer_findings(
    file: &str,
    source: &str,
    declared: &BTreeSet<String>,
) -> Vec<String> {
    let parsed = syn::parse_file(source).expect("the source parses");
    let inner: TokenStream = parsed
        .attrs
        .iter()
        .map(|attribute| attribute.to_token_stream())
        .collect();
    let mut found = silencers(inner);
    for item in &parsed.items {
        let Item::Mod(module) = item else {
            continue;
        };
        if !declared.contains(&unraw(&module.ident)) {
            continue;
        }
        let outer: TokenStream = module
            .attrs
            .iter()
            .map(|attribute| attribute.to_token_stream())
            .collect();
        found.extend(silencers(outer));
    }
    found
        .into_iter()
        .map(|silencer| format!("{file}: {silencer}"))
        .collect()
}

#[test]
fn den_and_commission_hold_no_lint_silencer() {
    let src = application_src();
    let guarded: BTreeSet<String> = den_production()
        .into_iter()
        .map(|(file, _)| file)
        .chain(act_files(&src))
        .collect();
    let read: Vec<&str> = guarded.iter().map(String::as_str).collect();

    let offenders = silencer_offenders(&src, &read);

    assert_eq!(offenders, Vec::<String>::new());
    for file in [
        "den.rs",
        GATE_FILE,
        "den/act.rs",
        "den/write.rs",
        "den/authority.rs",
        "commission/act.rs",
        "commission/create.rs",
        "commission/rename.rs",
        "commission/den.rs",
    ] {
        assert!(read.contains(&file), "the scan reads {file}: {read:?}");
    }
    let ancestors = ancestor_modules(&read);
    let ancestor_offenders: Vec<String> = ancestors
        .iter()
        .flat_map(|(file, declared)| {
            let source = std::fs::read_to_string(src.join(file)).expect("a readable source");
            ancestor_silencer_findings(file, &source, declared)
        })
        .collect();
    assert_eq!(ancestor_offenders, Vec::<String>::new());
    let named = |modules: &[&str]| -> BTreeSet<String> {
        modules.iter().map(|module| (*module).to_owned()).collect()
    };
    let expected_ancestors = BTreeMap::from([
        (
            "commission.rs".to_owned(),
            named(&["act", "create", "den", "rename"]),
        ),
        ("lib.rs".to_owned(), named(&["commission", "den"])),
    ]);
    assert_eq!(
        ancestors, expected_ancestors,
        "the scan reads lib.rs and commission.rs"
    );
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
    ];
    for source in refused {
        let found = silencer_findings("den/gate.rs", source);

        assert_eq!(found.len(), 1, "{source}: {found:?}");
    }
    let kept = [
        "#[allow(clippy::disallowed_methods, reason = \"a test opens its unit\")]\nfn opens() {}",
        "#[expect(clippy::disallowed_methods, reason = \"the gate's one node port\")]\nfn gate() {}",
        "#[cfg_attr(test, derive(Debug))]\nstruct Grant;",
        "/// Never `#[allow(dead_code)]` here.\nfn documented() {}",
        "fn named() { let allow = \"dead_code\"; }",
        "#[cfg(test)]\nmod tests;",
    ];
    for source in kept {
        let found = silencer_findings("den/gate.rs", source);

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

#[test]
fn the_act_files_are_every_handler_impl_and_the_floor() {
    let archive = "pub(crate) struct PlantedArchive;\n\n#[allow(dead_code)]\nfn later() {}\n\nimpl crate::den::Handler for PlantedArchive {}\n";
    let aliased = "use crate::den::Handler as Act;\n\nstruct Planted;\n\nimpl Act for Planted {}\n";
    let expanded = "macro_rules! act {\n    ($act:ident) => {\n        impl crate::den::Handler for $act {}\n    };\n}\n\nstruct Planted;\n\nact!(Planted);\n";
    let generic = "struct Planted<T>(T);\n\nimpl<T> crate::den::Handler<T> for Planted<T> {}\n";
    let commented = "// impl Handler for Planted {}\n/// impl Handler for Planted {}\nstruct Planted;\n\nconst NOTE: &str = \"impl Handler for Planted {}\";\n";
    let named = "fn takes(handler: &dyn crate::den::Handler) {}\n";
    let src = fixture_tree(
        "act_files",
        &[
            ("lib.rs", "mod commission;\nmod den;\n"),
            ("den.rs", "pub(crate) trait Handler {}\n"),
            (
                "commission.rs",
                "mod aliased;\nmod archive;\nmod commented;\nmod expanded;\nmod generic;\nmod named;\n",
            ),
            ("commission/archive.rs", archive),
            ("commission/aliased.rs", aliased),
            ("commission/expanded.rs", expanded),
            ("commission/generic.rs", generic),
            ("commission/commented.rs", commented),
            ("commission/named.rs", named),
        ],
    );

    let found = handler_files(&src);
    let read: Vec<&str> = found.iter().map(String::as_str).collect();
    let offenders = silencer_offenders(&src, &read);

    let expected: BTreeSet<String> = [
        "commission/aliased.rs",
        "commission/archive.rs",
        "commission/expanded.rs",
        "commission/generic.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(found, expected);
    assert_eq!(offenders, ["commission/archive.rs: allow(dead_code)"]);
    let with_floor = act_files(&src);
    for file in [
        "commission/archive.rs",
        "commission/act.rs",
        "commission/den.rs",
    ] {
        assert!(with_floor.contains(file), "{file}: {with_floor:?}");
    }
}

#[test]
fn the_modules_above_the_guarded_files_silence_none_of_them() {
    let library = BTreeSet::from(["commission".to_owned(), "den".to_owned()]);
    let commission = ["act", "create", "den", "rename"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<String>>();
    let refused = [
        ("lib.rs", &library, "#[allow(dead_code)]\npub mod den;"),
        ("lib.rs", &library, "#![allow(dead_code)]\npub mod den;"),
        (
            "lib.rs",
            &library,
            "#[expect(unused, reason = \"later\")]\npub mod commission;",
        ),
        (
            "lib.rs",
            &library,
            "#![cfg_attr(not(test), allow(warnings))]\npub mod account;",
        ),
        (
            "commission.rs",
            &commission,
            "#[cfg_attr(not(test), allow(dead_code))]\npub(crate) mod act;",
        ),
        (
            "commission.rs",
            &commission,
            "#[expect(unused)]\npub mod rename;",
        ),
        (
            "commission.rs",
            &commission,
            "#[allow(dead_code)]\npub mod create;",
        ),
        (
            "commission.rs",
            &commission,
            "#[allow(Dead_Code)]\npub(crate) mod r#den;",
        ),
        (
            "commission.rs",
            &commission,
            "#![expect(unused_variables, reason = \"later\")]\npub mod archive;",
        ),
    ];
    for (file, declared, source) in refused {
        let found = ancestor_silencer_findings(file, source, declared);

        assert_eq!(found.len(), 1, "{file}: {source}: {found:?}");
    }
    let sweep = "#[allow(\n    clippy::disallowed_methods,\n    reason = \"the sweep is a system job, not a use-case method\"\n)]\npub async fn sweep_deadlines() {}";
    let kept = [
        ("commission.rs", &commission, sweep),
        (
            "commission.rs",
            &commission,
            "#[allow(dead_code)]\nmod values;",
        ),
        ("lib.rs", &library, "#[cfg(test)]\nmod test_world;"),
        (
            "lib.rs",
            &library,
            "#[cfg_attr(test, derive(Debug))]\npub mod den;",
        ),
        (
            "lib.rs",
            &library,
            "//! Never `#![allow(dead_code)]` here.\n\npub mod den;",
        ),
    ];
    for (file, declared, source) in kept {
        let found = ancestor_silencer_findings(file, source, declared);

        assert_eq!(found, Vec::<String>::new(), "{file}: {source}");
    }
    let guarded = [
        "den.rs",
        "den/gate.rs",
        "den/read/walk.rs",
        "commission/act.rs",
    ];
    let expected = BTreeMap::from([
        (
            "commission.rs".to_owned(),
            BTreeSet::from(["act".to_owned()]),
        ),
        (
            "lib.rs".to_owned(),
            BTreeSet::from(["commission".to_owned(), "den".to_owned()]),
        ),
        (
            "den/read.rs".to_owned(),
            BTreeSet::from(["walk".to_owned()]),
        ),
    ]);
    assert_eq!(ancestor_modules(&guarded), expected);
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

/// The commission write view's hard delete: it takes no write capability, and clippy's ban on
/// `CommissionWrites::delete` holds its one call. It leaves when its method takes one.
const BANNED_WRITES: [&str; 1] = ["delete"];

/// The commission write view's other methods that take no write capability yet: seats,
/// elements, files, markups, the linked channel, view grants, and two reads on the write view.
/// Each leaves when its method takes one.
const OTHER_UNCAPPED: [&str; 12] = [
    "declare_seat",
    "create_seat_invitation",
    "revoke_seat_invitation",
    "add_element",
    "remove_element",
    "add_file",
    "add_markup",
    "set_linked_channel",
    "grant_view",
    "revoke_view",
    "commission_has_facts",
    "lapsed_deadlines",
];

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

/// Every name the three lists of uncapped methods hold.
fn listed_uncapped() -> BTreeSet<String> {
    FIELD_WRITES
        .into_iter()
        .chain(BANNED_WRITES)
        .chain(OTHER_UNCAPPED)
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

/// Whether `ty` is a reference to a path whose last name is [`WRITE_CAPABILITY`]: an alias, a
/// generic and a capability by value are not.
fn is_capability_reference(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    let Type::Path(path) = reference.elem.as_ref() else {
        return false;
    };
    let last = path.path.segments.last();
    path.qself.is_none() && last.is_some_and(|segment| segment.ident == WRITE_CAPABILITY)
}

/// Whether one of `signature`'s parameters is a write capability by reference.
fn takes_a_capability(signature: &syn::Signature) -> bool {
    signature.inputs.iter().any(
        |input| matches!(input, syn::FnArg::Typed(typed) if is_capability_reference(&typed.ty)),
    )
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

/// A trait's items sorted by authority: each method with a write capability by reference, and
/// everything else, an item that isn't a method included.
#[derive(Debug, Default)]
struct Authority {
    capped: BTreeSet<String>,
    uncapped: BTreeSet<String>,
}

/// `tr`'s items, sorted by authority.
fn authority(tr: &syn::ItemTrait) -> Authority {
    let mut sorted = Authority::default();
    for item in &tr.items {
        let capped = matches!(item, TraitItem::Fn(method) if takes_a_capability(&method.sig));
        let name = trait_item_name(item);
        if capped {
            sorted.capped.insert(name);
        } else {
            sorted.uncapped.insert(name);
        }
    }
    sorted
}

/// The port scan's findings in `file`: anything but one [`COMMISSION_WRITES`], each of its
/// uncapped items `listed` lacks, and each name in `listed` that isn't an uncapped item.
fn port_findings(file: &syn::File, listed: &BTreeSet<String>) -> Vec<String> {
    let found = traits_named(file, COMMISSION_WRITES);
    let [writes] = found.as_slice() else {
        let count = found.len();
        return vec![format!(
            "{count} `trait {COMMISSION_WRITES}` found, not one"
        )];
    };
    let sorted = authority(writes);
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
    unlisted.chain(stale).collect()
}

/// Each bound in `tr`'s supertraits, as written.
fn supertraits(tr: &syn::ItemTrait) -> Vec<String> {
    tr.supertraits
        .iter()
        .map(|bound| bound.to_token_stream().to_string())
        .collect()
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
    let sorted = authority(tr);
    let pinned: BTreeSet<String> = items.iter().map(|item| (*item).to_owned()).collect();
    let all: BTreeSet<String> = sorted.capped.union(&sorted.uncapped).cloned().collect();
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
    let writes = traits_named(file, COMMISSION_WRITES);
    let write_bounds: Vec<String> = writes
        .iter()
        .flat_map(|tr| {
            let mut found = Vec::new();
            let held = supertraits(tr);
            if held != ["Send"] {
                found.push(format!(
                    "trait {COMMISSION_WRITES}: supertraits {held:?}, not [\"Send\"]"
                ));
            }
            if !tr.generics.params.is_empty() || tr.generics.where_clause.is_some() {
                found.push(format!(
                    "trait {COMMISSION_WRITES}: generics or a where clause"
                ));
            }
            found
        })
        .collect();
    repo.into_iter().chain(reads).chain(write_bounds).collect()
}

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
    let sorted = authority(writes[0]);
    assert!(
        sorted.capped.contains("create"),
        "create takes the gate's create capability: {sorted:?}"
    );
    let list_lengths = FIELD_WRITES.len() + BANNED_WRITES.len() + OTHER_UNCAPPED.len();
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
    let capped = [
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: &WriteCapability<CommissionId, AsOwner>, step: LifecycleStep) -> anyhow::Result<bool>; }",
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, step: LifecycleStep, capability: &kernel::ports::WriteCapability<CommissionId, AsOwner>); }",
    ];
    for planted in capped {
        let file = syn::parse_file(planted).expect("the planted trait parses");

        let findings = port_findings(&file, &none);

        assert_eq!(findings, Vec::<String>::new(), "{planted}");
        let sorted = authority(traits_named(&file, COMMISSION_WRITES)[0]);
        assert!(
            sorted.capped.contains("set_lifecycle"),
            "{planted}: {sorted:?}"
        );
    }
    let uncapped = [
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: WriteCapability<CommissionId, AsOwner>); }",
        "type Cap = WriteCapability<CommissionId, AsOwner>;\ntrait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: &Cap); }",
        "trait CommissionWrites: Send { async fn set_lifecycle<C>(&mut self, capability: &C); }",
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: &impl AsRef<WriteCapability<CommissionId, AsOwner>>); }",
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, capability: &<Gate as Issue>::WriteCapability); }",
        "trait CommissionWrites: Send { async fn set_lifecycle(&mut self, id: &CommissionId) { let _ = WriteCapability::thing; } }",
    ];
    for planted in uncapped {
        let file = syn::parse_file(planted).expect("the planted trait parses");

        let findings = port_findings(&file, &none);

        assert_eq!(
            findings,
            ["CommissionWrites::set_lifecycle takes no write capability and is on no list"],
            "{planted}"
        );
    }
    let set_archived: BTreeSet<String> = BTreeSet::from(["set_archived".to_owned()]);
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
            "trait CommissionWrites: Send { async fn set_archived(&mut self, capability: &WriteCapability<CommissionId, AsOwner>); }",
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
