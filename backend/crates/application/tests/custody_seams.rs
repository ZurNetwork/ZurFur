//! Key custody's seams. The custody root and the 0.1 root key are named only where they are
//! read, derived from or opened; the custody migrator, which keeps a copy of the root, and
//! `connect_migrating`, which hands it back, only where they are built, called or run. None
//! is held as another struct's field (the migrator's 0.1 key aside), in a shared or locked
//! wrapper, or in a static. Only composition (and the test rig) links the HKDF
//! custodian, and of the driver crates only composition's custody file names the custodian,
//! where the boot guard runs before the one derivation. No custody file formats a value
//! through `Debug`, logs a field with the `?` sigil, or instruments a span with its fields.
//! The scans read production sources token by token, macro bodies included, and match whole
//! identifiers, aliases too.

mod seams;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use seams::{
    crates, drivers, identifiers, members_of, metadata, walk, with_aliases, workspace_production,
    workspace_sources,
};
use syn::{Expr, Item, Stmt};

/// A key type, or what carries one, the scan keeps in place: the files that may name it,
/// the folders whose every production file may, and the one struct that may hold it as a
/// field.
struct KeptKey {
    name: &'static str,
    files: &'static [&'static str],
    folders: &'static [&'static str],
    holder: Option<(&'static str, &'static str)>,
}

/// The custody root: composition reads, derives from and re-exports it, the drivers read
/// it from the environment, and the test rig plants its own.
const ROOT_SECRET: KeptKey = KeptKey {
    name: "RootSecret",
    files: &[
        "composition/src/root.rs",
        "composition/src/custody.rs",
        "composition/src/runtime.rs",
        "composition/src/lib.rs",
        "api/src/main.rs",
        "api/src/dev_profile.rs",
        "cli/src/lib.rs",
    ],
    folders: &["test-support/src"],
    holder: None,
};

/// The 0.1 root key: defined in the key vault, opened in the key store, held by the custody
/// migrator, and re-exported for the doctest that proves its constructor private.
const LEGACY_ROOT_KEY: KeptKey = KeptKey {
    name: "LegacyRootKey",
    files: &[
        "adapter-pg/src/key_vault.rs",
        "adapter-pg/src/key_store.rs",
        "adapter-pg/src/custody_migrator.rs",
        "adapter-pg/src/lib.rs",
    ],
    folders: &[],
    holder: Some(("adapter-pg/src/custody_migrator.rs", "CustodyMigrator")),
};

/// The custody migrator, which keeps a copy of the root until it runs: defined and
/// re-exported in adapter-pg, built in composition's runtime, and run by the CLI.
const CUSTODY_MIGRATOR: KeptKey = KeptKey {
    name: "CustodyMigrator",
    files: &[
        "adapter-pg/src/custody_migrator.rs",
        "adapter-pg/src/lib.rs",
        "composition/src/runtime.rs",
        "cli/src/lib.rs",
        "cli/src/commands/migrate.rs",
    ],
    folders: &["test-support/src"],
    holder: None,
};

/// The boot that hands back the custody migrator: defined in composition's runtime and
/// called by the drivers' boots, the CLI's through a wrapper of the same name.
const CONNECT_MIGRATING: KeptKey = KeptKey {
    name: "connect_migrating",
    files: &[
        "composition/src/runtime.rs",
        "api/src/main.rs",
        "cli/src/lib.rs",
    ],
    folders: &["test-support/src"],
    holder: None,
};

/// Every name the placement scan keeps in place.
const KEPT_KEYS: [KeptKey; 4] = [
    ROOT_SECRET,
    LEGACY_ROOT_KEY,
    CUSTODY_MIGRATOR,
    CONNECT_MIGRATING,
];

/// The shared and locked wrappers no kept name may sit inside.
const WRAPPERS: [&str; 4] = ["Arc", "Rc", "Mutex", "RwLock"];

/// The items whose bodies hold fields.
const FIELD_HOLDERS: [&str; 3] = ["struct", "enum", "union"];

/// The custodian port, its HKDF adapter, and that adapter's crate.
const CUSTODIAN_NAMES: [&str; 3] = ["KeyCustodian", "HkdfKeyCustodian", "adapter_hkdf"];

/// The one driver file that may name [`CUSTODIAN_NAMES`].
const CUSTODY_FILE: &str = "composition/src/custody.rs";

/// The HKDF custodian's crate, as cargo names it.
const ADAPTER_HKDF: &str = "adapter-hkdf";

/// The crates that may depend on [`ADAPTER_HKDF`] other than as a dev-dependency.
const HKDF_LINKERS: [&str; 2] = ["composition", "test-support"];

/// The one custody function, in [`CUSTODY_FILE`].
const ONE_CUSTODY_FUNCTION: &str = "guard_then_derive";

/// The boot guard on the raw root.
const GUARD: &str = "ensure_custody_hardened";

/// The HKDF custodian, whose `derive` is the one derivation.
const DERIVER: &str = "HkdfKeyCustodian";

/// The custody files the logging scan reads every production file of, from the crates folder.
const CUSTODY_FOLDERS: [&str; 3] = [
    "adapter-hkdf/src",
    "kernel/src/elements/custody",
    "kernel/src/ports/custody",
];

/// The single custody files the logging scan reads, from the crates folder; the CLI's
/// migrate command logs the custody migrator's errors.
const CUSTODY_FILES: [&str; 10] = [
    "kernel/src/elements/custody.rs",
    "kernel/src/ports/custody.rs",
    "adapter-pg/src/key_store.rs",
    "adapter-pg/src/key_vault.rs",
    "adapter-pg/src/custody_migrator.rs",
    "adapter-pg/src/lib.rs",
    "adapter-atproto/src/auth_store.rs",
    "composition/src/custody.rs",
    "composition/src/root.rs",
    "cli/src/commands/migrate.rs",
];

/// The tracing macros whose fields the `?` sigil would print through `Debug`.
const TRACING_MACROS: [&str; 12] = [
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "event",
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];

/// The span attribute, which must carry [`SKIP_ALL`].
const INSTRUMENT: &str = "instrument";

/// What keeps an instrumented function's arguments out of its span.
const SKIP_ALL: &str = "skip_all";

/// The macro that prints its argument through `Debug`.
const DBG: &str = "dbg";

/// tracing's field helper that records a value through `Debug`.
const DEBUG_FIELD: &str = "debug";

/// `path` from the crates folder, with `/` between its parts.
fn relative(path: &Path) -> String {
    let crates = crates();
    let inside = path.strip_prefix(&crates).unwrap_or(path);
    inside
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn parse(file: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("{file} parses: {error}"))
}

fn ident_at(level: &[TokenTree], at: usize) -> Option<String> {
    match level.get(at) {
        Some(TokenTree::Ident(ident)) => Some(syn::ext::IdentExt::unraw(ident).to_string()),
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

/// The group at `at`, when it has `delimiter`.
fn group_at(level: &[TokenTree], at: usize, delimiter: Delimiter) -> Option<TokenStream> {
    match level.get(at) {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter => Some(group.stream()),
        _ => None,
    }
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

/// The index past the generic arguments that open with `<` at `at`; the `>` of a `->`
/// closes nothing.
fn past_generics(level: &[TokenTree], at: usize) -> usize {
    let mut depth = 0usize;
    let mut index = at;
    while index < level.len() {
        let arrow = index > 0
            && matches!(
                level.get(index - 1),
                Some(TokenTree::Punct(punct)) if punct.as_char() == '-' && punct.spacing() == Spacing::Joint
            );
        if punct_at(level, index, '<') {
            depth += 1;
        } else if punct_at(level, index, '>') && !arrow {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return index + 1;
            }
        }
        index += 1;
    }
    index
}

/// Whether any identifier in `trees` is one of `names`.
fn names_any(trees: &[TokenTree], names: &BTreeSet<String>) -> bool {
    let tokens: TokenStream = trees.iter().cloned().collect();
    identifiers(tokens)
        .iter()
        .any(|ident| names.contains(ident))
}

// Where the root and the legacy key may live.

/// Whether `file` may name `key`.
fn may_name(key: &KeptKey, file: &str) -> bool {
    let in_folder = key
        .folders
        .iter()
        .any(|folder| file.starts_with(&format!("{folder}/")));
    key.files.contains(&file) || in_folder
}

/// Each field holder in `level` whose body names one of `names`, as `(kind, name)`: a
/// struct's, enum's or union's tokens past its name, up to its body or its `;`.
fn holders_naming(level: &[TokenTree], names: &BTreeSet<String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        let Some(kind) = ident_at(level, at) else {
            continue;
        };
        if !FIELD_HOLDERS.contains(&kind.as_str()) {
            continue;
        }
        let Some(holder) = ident_at(level, at + 1) else {
            continue;
        };
        let start = at + 2;
        let mut end = start;
        while end < level.len() {
            let body = group_at(level, end, Delimiter::Brace).is_some();
            if body || punct_at(level, end, ';') {
                end += 1;
                break;
            }
            end += 1;
        }
        if names_any(&level[start..end], names) {
            out.push((kind, holder));
        }
    }
    out
}

/// Each wrapper in `level` whose generic arguments name one of `names`: `Arc<…>`, or the
/// turbofish `Arc::<…>`.
fn wrappers_naming(level: &[TokenTree], names: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for at in 0..level.len() {
        let Some(wrapper) = ident_at(level, at) else {
            continue;
        };
        if !WRAPPERS.contains(&wrapper.as_str()) {
            continue;
        }
        let open = if path_separator_at(level, at + 1) {
            at + 3
        } else {
            at + 1
        };
        if !punct_at(level, open, '<') {
            continue;
        }
        let close = past_generics(level, open);
        if names_any(&level[open..close], names) {
            out.push(wrapper);
        }
    }
    out
}

/// Whether some `static` item in `level` (not the `'static` lifetime) names one of `names`
/// before its `;`.
fn static_naming(level: &[TokenTree], names: &BTreeSet<String>) -> bool {
    (0..level.len()).any(|at| {
        let is_static = ident_at(level, at).as_deref() == Some("static");
        let lifetime = at > 0 && punct_at(level, at - 1, '\'');
        if !is_static || lifetime {
            return false;
        }
        let end = (at..level.len())
            .find(|&index| punct_at(level, index, ';'))
            .unwrap_or(level.len());
        names_any(&level[at..end], names)
    })
}

/// What breaks `key`'s placement in `source`, the production file `file`: naming it (or
/// an alias of it, `names`) outside its files, holding it as another struct's field, in a
/// wrapper, or in a static.
fn key_findings(key: &KeptKey, file: &str, source: &str, names: &BTreeSet<String>) -> Vec<String> {
    let parsed = parse(file, source);
    let tokens = parsed.to_token_stream();
    let mut out = Vec::new();
    if !may_name(key, file) {
        let named: BTreeSet<String> = identifiers(tokens.clone())
            .into_iter()
            .filter(|ident| names.contains(ident))
            .collect();
        for name in named {
            out.push(format!("{file}: names `{name}`"));
        }
    }
    for level in levels(tokens) {
        for (kind, holder) in holders_naming(&level, names) {
            let allowed = key.holder == Some((file, holder.as_str()));
            if !allowed {
                out.push(format!("{file}: {kind} {holder} holds a `{}`", key.name));
            }
        }
        for wrapper in wrappers_naming(&level, names) {
            out.push(format!("{file}: a `{}` inside `{wrapper}<…>`", key.name));
        }
        if static_naming(&level, names) {
            out.push(format!("{file}: a `{}` in a static", key.name));
        }
    }
    out
}

#[test]
fn kept_names_stay_where_they_belong() {
    let sources = workspace_sources();
    let files: Vec<(String, String)> = workspace_production()
        .iter()
        .map(|path| (relative(path), read(path)))
        .collect();

    let mut offenders = Vec::new();
    let mut stale = Vec::new();
    for key in KEPT_KEYS {
        let names = with_aliases(&[key.name], &sources);
        for (file, source) in &files {
            offenders.extend(key_findings(&key, file, source, &names));
        }
        for allowed in key.files {
            let names_it =
                files
                    .iter()
                    .filter(|(file, _)| file == allowed)
                    .any(|(file, source)| {
                        let tokens = parse(file, source).to_token_stream();
                        identifiers(tokens).contains(&key.name.to_owned())
                    });
            if !names_it {
                stale.push(format!("{allowed} no longer names `{}`", key.name));
            }
        }
    }

    assert_eq!(offenders, Vec::<String>::new());
    assert_eq!(stale, Vec::<String>::new(), "an allowance outlived its use");
}

#[test]
fn the_key_scan_refuses_each_planted_hold() {
    let refused: [(&KeptKey, &str, &str); 17] = [
        (
            &ROOT_SECRET,
            "api/src/main.rs",
            "struct Boot { root: RootSecret }",
        ),
        (
            &ROOT_SECRET,
            "composition/src/runtime.rs",
            "pub struct Held(pub composition::RootSecret);",
        ),
        (
            &ROOT_SECRET,
            "composition/src/custody.rs",
            "enum Held { Root(RootSecret), None }",
        ),
        (
            &ROOT_SECRET,
            "composition/src/custody.rs",
            "fn share(root: RootSecret) -> Arc<RootSecret> { Arc::new(root) }",
        ),
        (
            &ROOT_SECRET,
            "cli/src/lib.rs",
            "fn share(root: RootSecret) { let shared = std::sync::Arc::<RootSecret>::new(root); }",
        ),
        (
            &ROOT_SECRET,
            "composition/src/root.rs",
            "static ROOT: OnceLock<RootSecret> = OnceLock::new();",
        ),
        (
            &ROOT_SECRET,
            "cli/src/commands/account.rs",
            "fn root() -> composition::RootSecret { todo!() }",
        ),
        (
            &LEGACY_ROOT_KEY,
            "composition/src/runtime.rs",
            "use adapter_pg::LegacyRootKey;",
        ),
        (
            &LEGACY_ROOT_KEY,
            "adapter-pg/src/key_store.rs",
            "fn keep(root: LegacyRootKey) -> Mutex<LegacyRootKey> { Mutex::new(root) }",
        ),
        (
            &LEGACY_ROOT_KEY,
            "adapter-pg/src/custody_migrator.rs",
            "struct Other { legacy_root: LegacyRootKey }",
        ),
        (
            &LEGACY_ROOT_KEY,
            "adapter-pg/src/key_vault.rs",
            "fn hold() { thread_local! { static ROOT: RefCell<Option<LegacyRootKey>> = RefCell::new(None); } }",
        ),
        (
            &CUSTODY_MIGRATOR,
            "api/src/main.rs",
            "struct KeptMigrator(std::sync::Arc<adapter_pg::CustodyMigrator>);",
        ),
        (
            &CUSTODY_MIGRATOR,
            "cli/src/lib.rs",
            "static MIGRATOR: OnceLock<CustodyMigrator> = OnceLock::new();",
        ),
        (
            &CUSTODY_MIGRATOR,
            "composition/src/runtime.rs",
            "fn keep(migrator: CustodyMigrator) -> Mutex<CustodyMigrator> { Mutex::new(migrator) }",
        ),
        (
            &CUSTODY_MIGRATOR,
            "cli/src/commands/migrate.rs",
            "pub struct Migrating { migrator: CustodyMigrator, args: MigrateArgs }",
        ),
        (
            &CONNECT_MIGRATING,
            "cli/src/commands/account.rs",
            "fn boot() { let _connect = composition::Runtime::connect_migrating; }",
        ),
        (
            &CONNECT_MIGRATING,
            "api/src/main.rs",
            "static BOOT: fn(Config, RootSecret, &[Purpose]) -> Connecting = AppState::connect_migrating;",
        ),
    ];
    for (key, file, source) in refused {
        let names = with_aliases(&[key.name], &[source.to_owned()]);

        let found = key_findings(key, file, source, &names);

        assert!(!found.is_empty(), "{file}: {source}");
    }

    let aliased: [(&KeptKey, &str, &str); 2] = [
        (
            &ROOT_SECRET,
            "api/src/main.rs",
            "use composition::RootSecret as Root;\nstruct Boot { root: Root }",
        ),
        (
            &CUSTODY_MIGRATOR,
            "cli/src/lib.rs",
            "use adapter_pg::CustodyMigrator as Kept;\nstruct Boot { kept: Kept }",
        ),
    ];
    for (key, file, source) in aliased {
        let names = with_aliases(&[key.name], &[source.to_owned()]);

        let found = key_findings(key, file, source, &names);

        assert!(!found.is_empty(), "{file}: {source}");
    }

    let allowed: [(&KeptKey, &str, &str); 8] = [
        (
            &ROOT_SECRET,
            "composition/src/custody.rs",
            "fn derive(root: RootSecret, purposes: &[Purpose]) -> Custody { todo!() }",
        ),
        (
            &ROOT_SECRET,
            "test-support/src/pg_app/served.rs",
            "fn serve(root: RootSecret) -> &'static str { \"served\" }",
        ),
        (
            &ROOT_SECRET,
            "api/src/main.rs",
            "struct Boot { name: &'static str, purposes: Arc<[Purpose]> }",
        ),
        (
            &LEGACY_ROOT_KEY,
            "adapter-pg/src/custody_migrator.rs",
            "pub struct CustodyMigrator { pool: PgPool, legacy_root: LegacyRootKey }",
        ),
        (
            &LEGACY_ROOT_KEY,
            "adapter-pg/src/key_vault.rs",
            "pub struct LegacyRootKey(Zeroizing<[u8; 32]>);\nimpl LegacyRootKey { pub(crate) fn new(root: Zeroizing<[u8; 32]>) -> Self { Self(root) } }",
        ),
        (
            &CUSTODY_MIGRATOR,
            "adapter-pg/src/custody_migrator.rs",
            "pub struct CustodyMigrator { pool: PgPool, legacy_root: LegacyRootKey, thread_bound: PhantomData<*const ()> }",
        ),
        (
            &CUSTODY_MIGRATOR,
            "cli/src/commands/migrate.rs",
            "pub async fn run(custody_migrator: CustodyMigrator, args: MigrateArgs) -> Result<Value, CliError> { todo!() }",
        ),
        (
            &CONNECT_MIGRATING,
            "api/src/main.rs",
            "async fn profile_app(config: Config) -> anyhow::Result<()> { let (app_state, custody_migrator) = AppState::connect_migrating(config, root, &purposes).await?; Ok(()) }",
        ),
    ];
    for (key, file, source) in allowed {
        let names = with_aliases(&[key.name], &[source.to_owned()]);

        let found = key_findings(key, file, source, &names);

        assert_eq!(found, Vec::<String>::new(), "{file}: {source}");
    }
}

// Only composition derives.

#[test]
fn only_composition_links_adapter_hkdf() {
    let members = members_of(&metadata());

    let linkers: BTreeSet<String> = members
        .iter()
        .filter(|member| member.depends_normally_on(ADAPTER_HKDF))
        .map(|member| member.name.clone())
        .collect();

    let expected: BTreeSet<String> = HKDF_LINKERS.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(linkers, expected);
}

/// Each naming of the custodian (or an alias of it, `names`) in `source`, the driver file
/// `file`; none in [`CUSTODY_FILE`].
fn custodian_findings(file: &str, source: &str, names: &BTreeSet<String>) -> Vec<String> {
    if file == CUSTODY_FILE {
        return Vec::new();
    }
    let tokens = parse(file, source).to_token_stream();
    identifiers(tokens)
        .into_iter()
        .filter(|ident| names.contains(ident))
        .map(|ident| format!("{file}: names `{ident}`"))
        .collect()
}

/// Each driver crate's production files, as paths from the crates folder.
fn driver_files() -> Vec<(String, PathBuf)> {
    drivers()
        .iter()
        .flat_map(|member| walk(&member.folder.join("src")).production)
        .map(|path| (relative(&path), path))
        .collect()
}

#[test]
fn only_the_custody_file_names_the_custodian() {
    let names = with_aliases(&CUSTODIAN_NAMES, &workspace_sources());
    let files = driver_files();

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|(file, path)| custodian_findings(file, &read(path), &names))
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let read_the_custody_file = files.iter().any(|(file, _)| file == CUSTODY_FILE);
    assert!(read_the_custody_file, "the scan reads {CUSTODY_FILE}");
    for driver in ["composition", "api", "cli"] {
        let read_it = files
            .iter()
            .any(|(file, _)| file.starts_with(&format!("{driver}/src/")));
        assert!(read_it, "the scan reads {driver}");
    }
}

#[test]
fn the_custodian_scan_refuses_each_planted_naming() {
    let refused = [
        ("api/src/main.rs", "use adapter_hkdf as h;"),
        (
            "api/src/dev_profile.rs",
            "fn mac() -> Arc<dyn KeyCustodian> { todo!() }",
        ),
        (
            "cli/src/lib.rs",
            "fn derive(root: Zeroizing<[u8; 32]>) { let custodian = adapter_hkdf::HkdfKeyCustodian::derive(root, &[]); }",
        ),
        (
            "composition/src/runtime.rs",
            "use domain::ports::KeyCustodian;",
        ),
        (
            "composition/src/custody/more.rs",
            "extern crate adapter_hkdf;",
        ),
    ];
    for (file, source) in refused {
        let names = with_aliases(&CUSTODIAN_NAMES, &[source.to_owned()]);

        let found = custodian_findings(file, source, &names);

        assert!(!found.is_empty(), "{file}: {source}");
    }

    let aliased = "use adapter_hkdf::HkdfKeyCustodian as Keys;";
    let sources = [aliased.to_owned()];
    let names = with_aliases(&CUSTODIAN_NAMES, &sources);
    let used = "fn derive(root: Zeroizing<[u8; 32]>) -> Keys { Keys::derive(root, &[]) }";
    let found = custodian_findings("api/src/dev_profile.rs", used, &names);
    assert!(!found.is_empty(), "{used}");

    let allowed = [
        (
            CUSTODY_FILE,
            "use adapter_hkdf::HkdfKeyCustodian;\nfn handles(custodian: Arc<dyn KeyCustodian>) {}",
        ),
        (
            "api/src/dev_profile.rs",
            "/// Derived by the HKDF custodian (KeyCustodian) in composition.\nfn den_mac(custody: &Custody) -> ScopedMac { custody.den_mac().clone() }",
        ),
    ];
    for (file, source) in allowed {
        let names = with_aliases(&CUSTODIAN_NAMES, &[source.to_owned()]);

        let found = custodian_findings(file, source, &names);

        assert_eq!(found, Vec::<String>::new(), "{file}: {source}");
    }
}

/// Whether `tokens` hold the identifier `name`.
fn holds_ident(tokens: TokenStream, name: &str) -> bool {
    identifiers(tokens).iter().any(|ident| ident == name)
}

/// Whether `statement` is the guard's call ended by `?`, so a refusal returns before the
/// statements after it.
fn returns_on_refusal(statement: &Stmt) -> bool {
    let Stmt::Expr(Expr::Try(tried), Some(_)) = statement else {
        return false;
    };
    let Expr::Call(call) = &*tried.expr else {
        return false;
    };
    holds_ident(call.func.to_token_stream(), GUARD)
}

/// What breaks the one custody function in `source`, the custody file: the guard and the
/// derivation each named once outside imports, both inside [`ONE_CUSTODY_FUNCTION`], the
/// guard a statement of its own that returns on a refusal, before the derivation's.
fn guard_findings(source: &str) -> Vec<String> {
    let parsed = parse(CUSTODY_FILE, source);
    let mut out = Vec::new();
    let outside_imports: TokenStream = parsed
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Use(_)))
        .map(ToTokens::to_token_stream)
        .collect();
    let idents = identifiers(outside_imports);
    for name in [GUARD, DERIVER] {
        let count = idents.iter().filter(|ident| *ident == name).count();
        if count != 1 {
            out.push(format!(
                "`{name}` named {count} times outside imports, not once"
            ));
        }
    }
    let functions: Vec<&syn::ItemFn> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.sig.ident == ONE_CUSTODY_FUNCTION => Some(function),
            _ => None,
        })
        .collect();
    let [function] = functions.as_slice() else {
        out.push(format!(
            "{} functions named `{ONE_CUSTODY_FUNCTION}`, not one",
            functions.len()
        ));
        return out;
    };
    let statements = &function.block.stmts;
    let guard = statements
        .iter()
        .position(|statement| holds_ident(statement.to_token_stream(), GUARD));
    let derivation = statements
        .iter()
        .position(|statement| holds_ident(statement.to_token_stream(), DERIVER));
    let (Some(guard), Some(derivation)) = (guard, derivation) else {
        out.push(format!(
            "`{ONE_CUSTODY_FUNCTION}` holds the guard at {guard:?} and the derivation at {derivation:?}"
        ));
        return out;
    };
    if !returns_on_refusal(&statements[guard]) {
        out.push("the guard is not a call ended by `?`".to_owned());
    }
    if guard >= derivation {
        out.push("the derivation runs before the guard".to_owned());
    }
    out
}

#[test]
fn the_guard_runs_inside_the_one_custody_function() {
    let source = read(&crates().join(CUSTODY_FILE));

    let found = guard_findings(&source);

    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn the_guard_scan_refuses_each_planted_order() {
    let guarded = "fn guard_then_derive(root: Root, env: &Environment) -> anyhow::Result<Custody> {\n    ensure_custody_hardened(env, root.as_slice(), false)?;\n    let derived = HkdfKeyCustodian::derive(root, purposes);\n    Ok(handles(Arc::new(derived)))\n}";
    let refused = [
        "fn guard_then_derive(root: Root, env: &Environment) -> anyhow::Result<Custody> {\n    let derived = HkdfKeyCustodian::derive(root, purposes);\n    ensure_custody_hardened(env, derived.as_slice(), false)?;\n    Ok(handles(Arc::new(derived)))\n}",
        "fn guard_then_derive(root: Root, env: &Environment) -> anyhow::Result<Custody> {\n    let _ = ensure_custody_hardened(env, root.as_slice(), false);\n    let derived = HkdfKeyCustodian::derive(root, purposes);\n    Ok(handles(Arc::new(derived)))\n}",
        "fn guard_then_derive(root: Root) -> anyhow::Result<Custody> {\n    let derived = HkdfKeyCustodian::derive(root, purposes);\n    Ok(handles(Arc::new(derived)))\n}",
        "fn derive_unguarded(root: Root) -> HkdfKeyCustodian { todo!() }",
    ];
    let with_a_second_derivation = format!(
        "{guarded}\nfn raw(root: Root) -> Custody {{ handles(Arc::new(HkdfKeyCustodian::derive(root, &[]))) }}"
    );
    let with_a_second_guard = format!(
        "{guarded}\nfn check(env: &Environment) {{ ensure_custody_hardened(env, &[], false).ok(); }}"
    );

    assert_eq!(guard_findings(guarded), Vec::<String>::new());
    for source in refused {
        assert!(!guard_findings(source).is_empty(), "{source}");
    }
    for source in [with_a_second_derivation, with_a_second_guard] {
        assert!(!guard_findings(&source).is_empty(), "{source}");
    }
}

// Nothing secret logged.

/// The custody files' production sources, each with its path from the crates folder.
fn custody_files() -> Vec<(String, String)> {
    let crates_with_custody: BTreeSet<&str> = CUSTODY_FOLDERS
        .iter()
        .chain(CUSTODY_FILES.iter())
        .filter_map(|path| path.split('/').next())
        .collect();
    crates_with_custody
        .iter()
        .flat_map(|krate| walk(&crates().join(krate).join("src")).production)
        .map(|path| (relative(&path), path))
        .filter(|(file, _)| {
            let in_folder = CUSTODY_FOLDERS
                .iter()
                .any(|folder| file.starts_with(&format!("{folder}/")));
            in_folder || CUSTODY_FILES.contains(&file.as_str())
        })
        .map(|(file, path)| (file, read(&path)))
        .collect()
}

/// Whether the group at `at` (after `#` or `#!`) is an attribute's.
fn attribute_at(level: &[TokenTree], at: usize) -> bool {
    let after_hash = punct_at(level, at.wrapping_sub(1), '#')
        || (punct_at(level, at.wrapping_sub(1), '!') && punct_at(level, at.wrapping_sub(2), '#'));
    after_hash && group_at(level, at, Delimiter::Bracket).is_some()
}

/// Whether the attribute group `tokens` is a doc comment.
fn is_doc(tokens: &TokenStream) -> bool {
    let first = tokens.clone().into_iter().next();
    matches!(first, Some(TokenTree::Ident(ident)) if ident == "doc")
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

/// Every string literal in `tokens` outside doc comments.
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
        if attribute_at(&level, at) && is_doc(&group.stream()) {
            continue;
        }
        out.extend(string_literals(group.stream()));
    }
    out
}

/// The format specs in `text` that print through `Debug`: each `{…:…?}`, `{{` escapes aside.
fn debug_specs(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < characters.len() {
        if characters[at] != '{' {
            at += 1;
            continue;
        }
        if characters.get(at + 1) == Some(&'{') {
            at += 2;
            continue;
        }
        let Some(length) = characters[at..]
            .iter()
            .position(|character| *character == '}')
        else {
            break;
        };
        let inside: String = characters[at + 1..at + length].iter().collect();
        let debug = inside
            .split_once(':')
            .is_some_and(|(_, spec)| spec.ends_with('?'));
        if debug {
            out.push(format!("{{{inside}}}"));
        }
        at += length + 1;
    }
    out
}

/// Whether the `?` at `at` is tracing's `Debug` sigil: it opens a field, after nothing, a
/// `,` or an `=`. The `?` operator follows an expression instead.
fn sigil_at(level: &[TokenTree], at: usize) -> bool {
    punct_at(level, at, '?')
        && (at == 0 || punct_at(level, at - 1, ',') || punct_at(level, at - 1, '='))
}

/// What prints through `Debug` inside `tokens`, a tracing macro's or an `instrument`'s
/// arguments: a `?` sigil, or tracing's `debug(…)` field helper.
fn debug_fields(tokens: TokenStream) -> Vec<String> {
    let mut out = Vec::new();
    for level in levels(tokens) {
        for at in 0..level.len() {
            if sigil_at(&level, at) {
                out.push("a `?` field".to_owned());
            }
            let helper = ident_at(&level, at).as_deref() == Some(DEBUG_FIELD)
                && group_at(&level, at + 1, Delimiter::Parenthesis).is_some();
            if helper {
                out.push("a `debug(…)` field".to_owned());
            }
        }
    }
    out
}

/// Each `instrument` in the attribute group `tokens` (or an alias of it, `instruments`)
/// without `skip_all`, or with a field it prints through `Debug`.
fn instrument_findings(tokens: TokenStream, instruments: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for level in levels(tokens) {
        for at in 0..level.len() {
            let Some(ident) = ident_at(&level, at) else {
                continue;
            };
            let method = at > 0 && punct_at(&level, at - 1, '.');
            if !instruments.contains(&ident) || method {
                continue;
            }
            let arguments = group_at(&level, at + 1, Delimiter::Parenthesis);
            let skips_all = arguments.as_ref().is_some_and(|arguments| {
                arguments
                    .clone()
                    .into_iter()
                    .any(|tree| matches!(&tree, TokenTree::Ident(word) if word == SKIP_ALL))
            });
            if !skips_all {
                out.push(format!("`{ident}` without `{SKIP_ALL}`"));
            }
            if let Some(arguments) = arguments {
                for field in debug_fields(arguments) {
                    out.push(format!("`{ident}` with {field}"));
                }
            }
        }
    }
    out
}

/// What prints through `Debug` in `source`, the custody file `file`: a `Debug` format
/// spec in any string literal but a doc comment, `dbg!`, a tracing macro's `?` or
/// `debug(…)` field, and an `instrument` without `skip_all` or with such a field.
/// `instruments` and `macros` are the attribute's and the macros' names, aliases included.
fn logging_findings(
    file: &str,
    source: &str,
    instruments: &BTreeSet<String>,
    macros: &BTreeSet<String>,
) -> Vec<String> {
    let tokens = parse(file, source).to_token_stream();
    let mut out = Vec::new();
    for text in string_literals(tokens.clone()) {
        for spec in debug_specs(&text) {
            out.push(format!("{file}: `{spec}` formats through Debug"));
        }
    }
    for level in levels(tokens) {
        for at in 0..level.len() {
            if attribute_at(&level, at) {
                let attribute = group_at(&level, at, Delimiter::Bracket).unwrap_or_default();
                for finding in instrument_findings(attribute, instruments) {
                    out.push(format!("{file}: {finding}"));
                }
                continue;
            }
            let Some(name) = ident_at(&level, at) else {
                continue;
            };
            let invoked = punct_at(&level, at + 1, '!');
            let Some(TokenTree::Group(arguments)) = level.get(at + 2) else {
                continue;
            };
            if !invoked {
                continue;
            }
            if name == DBG {
                out.push(format!("{file}: `dbg!`"));
            }
            if macros.contains(&name) {
                for field in debug_fields(arguments.stream()) {
                    out.push(format!("{file}: `{name}!` with {field}"));
                }
            }
        }
    }
    out
}

/// The names the logging scan matches: `instrument` and the tracing macros, each with the
/// aliases `sources` bind to it.
fn logging_names(sources: &[String]) -> (BTreeSet<String>, BTreeSet<String>) {
    let instruments = with_aliases(&[INSTRUMENT], sources);
    let macros = with_aliases(&TRACING_MACROS, sources);
    (instruments, macros)
}

#[test]
fn custody_files_never_debug_format_or_instrument_fields() {
    let files = custody_files();
    let sources: Vec<String> = files.iter().map(|(_, source)| source.clone()).collect();
    let (instruments, macros) = logging_names(&sources);

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|(file, source)| logging_findings(file, source, &instruments, &macros))
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let read: BTreeSet<&str> = files.iter().map(|(file, _)| file.as_str()).collect();
    for file in CUSTODY_FILES {
        assert!(read.contains(file), "the scan reads {file}");
    }
    for folder in ["adapter-hkdf/src", "kernel/src/elements/custody"] {
        let read_it = read
            .iter()
            .any(|file| file.starts_with(&format!("{folder}/")));
        assert!(read_it, "the scan reads {folder}");
    }
}

#[test]
fn the_logging_scan_refuses_each_planted_print() {
    let refused = [
        "#[tracing::instrument]\nasync fn put(&self, did: &Did, keys: &AccountKeys) {}",
        "#[tracing::instrument(level = \"debug\")]\nasync fn get(&self, did: &Did) {}",
        "#[tracing::instrument(level = \"debug\", skip_all, fields(keys = ?keys))]\nasync fn put(&self, keys: &AccountKeys) {}",
        "#[instrument(skip(self))]\nasync fn put(&self, keys: &AccountKeys) {}",
        "fn seal(plaintext: &[u8]) { tracing::debug!(?plaintext); }",
        "fn seal(plaintext: &[u8]) { debug!(bytes = ?plaintext, \"sealed\"); }",
        "fn seal(plaintext: &[u8]) { tracing::info!(target: \"custody\", len = plaintext.len(), ?plaintext); }",
        "fn seal(key: &[u8]) { tracing::event!(Level::TRACE, key = tracing::field::debug(key)); }",
        "fn seal(key: &[u8]) { let span = tracing::trace_span!(\"seal\", key = ?key); }",
        "fn shown(key: &[u8]) -> String { format!(\"{:?}\", key) }",
        "fn shown(key: &[u8]) -> String { format!(\"{key:#?}\") }",
        "fn shown(key: &[u8]) -> String { format!(\"{0:x?}\", key) }",
        "fn shown(f: &mut fmt::Formatter<'_>, key: &[u8]) -> fmt::Result { write!(f, \"key {:?}\", key) }",
        "#[derive(Debug, thiserror::Error)]\n#[error(\"a key {0:?} failed\")]\npub struct Failed(Vec<u8>);",
        "fn shown(key: &[u8]) { dbg!(key); }",
        "fn refusal(error: CustodyMigrationError) { tracing::debug!(error = ?error, \"the custody migration stopped\"); }",
        "use tracing::instrument as traced;\n#[traced]\nfn put() {}",
        "use tracing::debug as note;\nfn seal(plaintext: &[u8]) { note!(?plaintext); }",
    ];
    for source in refused {
        let (instruments, macros) = logging_names(&[source.to_owned()]);

        let found = logging_findings("adapter-pg/src/key_store.rs", source, &instruments, &macros);

        assert!(!found.is_empty(), "{source}");
    }

    let allowed = [
        "#[tracing::instrument(level = \"debug\", skip_all)]\nasync fn put(&self, keys: &AccountKeys) {}",
        "fn probe(row: &Row, error: &ResealError) { tracing::debug!(did = %row.did, %error, \"a custody row fails the dry run\"); }",
        "fn open(bytes: &[u8]) -> Result<usize, E> { let opened = parse(bytes)?; tracing::debug!(len = opened.len(), \"opened\"); Ok(opened.len()) }",
        "/// Never prints `{:?}` of a key.\nfn shown() {}",
        "fn shown() -> String { format!(\"{{:?}} {}\", 1) }",
        "fn spanned(work: impl Future) { work.instrument(tracing::debug_span!(\"work\")); }",
        "fn shown(f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct(\"Key\").finish_non_exhaustive() }",
        "#[error(\"{}\", unopenable_rows(.count))]\nstruct Unopenable { count: usize }",
    ];
    for source in allowed {
        let (instruments, macros) = logging_names(&[source.to_owned()]);

        let found = logging_findings("adapter-pg/src/key_store.rs", source, &instruments, &macros);

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}
