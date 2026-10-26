//! The whole-row commission read (field, trait and aliases) is named only where listed, one
//! read each in `get` and `clear_channel`; every commissions route naming a commission id has
//! its row in test-support's call table and no more; and test-support reaches adapter-pg's raw
//! commission queries only in the den world's two seeders. Scans read token by token.

mod seams;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use seams::{
    Scope, crates, fixture_tree, ident_at, identifiers, levels, path_separator_at, punct_at,
    relative_to_crates, scopes, with_aliases, workspace_production, workspace_sources,
};
use syn::{Expr, Item, ItemFn, Lit, Stmt};
use test_support::pg_app::{CommissionCall, absent_commission, every_commission_call};

/// A naming a scan found: the scope it sits in and what it names.
type Naming = (String, String);

/// Each of `found` past the allowance `allowed` gives its `(scope, name)`.
fn past_allowance(found: Vec<Naming>, allowed: &[(&str, &str, usize)]) -> Vec<Naming> {
    let mut left: BTreeMap<Naming, usize> = allowed
        .iter()
        .map(|(scope, name, count)| (((*scope).to_owned(), (*name).to_owned()), *count))
        .collect();
    found
        .into_iter()
        .filter(|naming| {
            let Some(count) = left.get_mut(naming).filter(|count| **count > 0) else {
                return true;
            };
            *count -= 1;
            false
        })
        .collect()
}

fn parsed(file: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("{file} parses: {error}"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

// The whole-row read.

/// The whole-row read's pool field and its port trait.
const WHOLE_ROW_READ: [&str; 2] = ["commission_rows", "CommissionRows"];

/// The files that may name the whole-row read anywhere: the port's definition and its export.
const PORT_FILES: [&str; 2] = ["workflow/src/ports/commission.rs", "workflow/src/ports.rs"];

/// The folders whose every file may name it: the adapters' impls, the composition root's
/// wiring, and the test rig.
const OPEN_FOLDERS: [&str; 4] = [
    "adapter-pg/src/",
    "adapter-mem/src/",
    "composition/src/",
    "test-support/src/",
];

/// The commission read's file, and the scope in it that reads the whole row once.
const GET_FILE: &str = "application/src/commission/get.rs";
const GET: &str = "Commissions::get";

/// The channel's route file, and the handler in it that reads the whole row once.
const CHANNEL_FILE: &str = "api/src/routes/commissions/channel.rs";
const CLEAR_CHANNEL: &str = "clear_channel";

/// Every other place the whole-row read is named, by file, scope and name, each named exactly
/// the pinned number of times: `Ports`' import and field, the dev profile's hand-built
/// runtime, and the two reads.
const READ_SITES: [(&str, &str, &str, usize); 6] = [
    ("application/src/app.rs", "use", "CommissionRows", 1),
    (
        "application/src/app.rs",
        "struct Ports",
        "CommissionRows",
        1,
    ),
    (
        "application/src/app.rs",
        "struct Ports",
        "commission_rows",
        1,
    ),
    ("api/src/dev_profile.rs", "runtime", "commission_rows", 2),
    (GET_FILE, GET, "commission_rows", 1),
    (CHANNEL_FILE, CLEAR_CHANNEL, "commission_rows", 1),
];

/// The whole-row read's names, and every name `sources` bind to one, through any chain.
fn whole_row_names(sources: &[String]) -> BTreeSet<String> {
    with_aliases(&WHOLE_ROW_READ, sources)
}

/// Each naming of `names` in `source`, the file at `file` under the crates folder, past what
/// that file may name.
fn whole_row_findings(file: &str, source: &str, names: &BTreeSet<String>) -> Vec<String> {
    let open =
        PORT_FILES.contains(&file) || OPEN_FOLDERS.iter().any(|folder| file.starts_with(folder));
    if open {
        return Vec::new();
    }
    let found: Vec<Naming> = scopes(&parsed(file, source))
        .into_iter()
        .flat_map(|Scope { context, tokens }| {
            identifiers(tokens)
                .into_iter()
                .filter(|ident| names.contains(ident))
                .map(move |ident| (context.clone(), ident))
        })
        .collect();
    let allowed: Vec<(&str, &str, usize)> = READ_SITES
        .iter()
        .filter(|(site, ..)| *site == file)
        .map(|(_, scope, name, count)| (*scope, *name, *count))
        .collect();
    past_allowance(found, &allowed)
        .into_iter()
        .map(|(scope, name)| format!("{file}: `{name}` in {scope}"))
        .collect()
}

/// How often the scope `context` of the file at `file` names `name`.
fn named_in(file: &str, context: &str, name: &str) -> usize {
    let source = read(&crates().join(file));
    scopes(&parsed(file, &source))
        .into_iter()
        .filter(|scope| scope.context == context)
        .map(|scope| {
            identifiers(scope.tokens)
                .iter()
                .filter(|ident| *ident == name)
                .count()
        })
        .sum()
}

#[test]
fn the_whole_commission_read_is_named_only_where_listed() {
    let names = whole_row_names(&workspace_sources());
    let production = workspace_production();

    let offenders: Vec<String> = production
        .iter()
        .flat_map(|path| {
            let file = relative_to_crates(path);
            whole_row_findings(&file, &read(path), &names)
        })
        .collect();

    assert_eq!(offenders, Vec::<String>::new());
    let read_sites: Vec<&str> = READ_SITES.iter().map(|(file, ..)| *file).collect();
    for file in read_sites.iter().chain(&PORT_FILES) {
        let path = crates().join(file);
        assert!(production.contains(&path), "{file} is not read");
    }
    for (file, scope, name, count) in READ_SITES {
        let named = named_in(file, scope, name);

        assert_eq!(named, count, "{file}: `{name}` in {scope}");
    }
}

#[test]
fn the_whole_row_scan_refuses_each_way_around_the_two_reads() {
    let names = whole_row_names(&[]);
    let declare = "application/src/commission/seats/declare.rs";
    let refused = [
        (
            declare,
            "async fn declare(ports: &Ports, id: CommissionId) { let rows = &ports.commission_rows; rows.find(&id).await; }",
        ),
        (
            declare,
            "async fn declare(ports: &Ports, id: CommissionId) { CommissionRows::find(&*ports.commission_rows, &id).await; }",
        ),
        (
            "application/src/commission/rows.rs",
            "pub fn rows(ports: &Ports) -> &dyn CommissionRows { &*ports.commission_rows }",
        ),
        (
            "application/src/commission/rows.rs",
            "pub fn rows<R: domain::ports::CommissionRows>(rows: R) -> R { rows }",
        ),
        (
            GET_FILE,
            "impl Commissions<'_> { async fn other(&self, ports: &Ports, id: CommissionId) { ports.commission_rows.find(&id).await; } }",
        ),
        (
            GET_FILE,
            "async fn get(ports: &Ports, id: CommissionId) { ports.commission_rows.find(&id).await; }",
        ),
        (
            GET_FILE,
            "mod inner { impl Commissions<'_> { async fn get(&self, ports: &Ports, id: CommissionId) { ports.commission_rows.find(&id).await; } } }",
        ),
        (
            CHANNEL_FILE,
            "async fn link_channel(State(state): State<AppState>, id: CommissionId) { state.commission_rows.find(&id).await; }",
        ),
        (
            "api/src/routes/commissions/get.rs",
            "async fn get_commission(State(state): State<AppState>, id: CommissionId) { state.commission_rows.find(&id).await; }",
        ),
        (
            "application/src/app.rs",
            "impl App { fn rows(&self) -> &dyn CommissionRows { &*self.ports.commission_rows } }",
        ),
        (
            "cli/src/lib.rs",
            "macro_rules! reach { ($ports:expr) => { $ports.commission_rows.find(&id) }; }",
        ),
    ];

    for (file, source) in refused {
        let found = whole_row_findings(file, source, &names);

        assert!(!found.is_empty(), "{file}: {source}");
    }
}

#[test]
fn a_second_read_inside_get_or_clear_channel_is_refused() {
    let names = whole_row_names(&[]);
    let twice_in_get = "impl Commissions<'_> { #[use_case] pub async fn get(&self, #[ports] ports: &Ports, id: CommissionId) { ports.commission_rows.find(&id).await; CommissionRows::find(&*ports.commission_rows, &id).await; } }";
    let twice_in_clear = "async fn clear_channel(State(state): State<AppState>, id: CommissionId) { state.commission_rows.find(&id).await; state.commission_rows.find(&id).await; }";

    let in_get = whole_row_findings(GET_FILE, twice_in_get, &names);
    let in_clear = whole_row_findings(CHANNEL_FILE, twice_in_clear, &names);

    let expected_in_get = [
        format!("{GET_FILE}: `CommissionRows` in {GET}"),
        format!("{GET_FILE}: `commission_rows` in {GET}"),
    ];
    let expected_in_clear = [format!(
        "{CHANNEL_FILE}: `commission_rows` in {CLEAR_CHANNEL}"
    )];
    assert_eq!(in_get, expected_in_get);
    assert_eq!(in_clear, expected_in_clear);
}

#[test]
fn an_alias_of_the_whole_row_port_is_refused_where_it_is_used() {
    let alias = "pub use domain::ports::CommissionRows as Rows;".to_owned();
    let chained = "pub type Pool = dyn Rows;".to_owned();
    let names = whole_row_names(&[alias.clone(), chained]);
    let declare = "application/src/commission/seats/declare.rs";
    let used = [
        "fn reach(rows: &dyn Rows) {}",
        "fn reach(rows: &Pool) {}",
        "async fn reach(rows: &dyn crate::commission::Rows, id: CommissionId) { Rows::find(rows, &id).await; }",
    ];

    let at_the_alias = whole_row_findings("application/src/commission/rows.rs", &alias, &names);

    assert!(
        names.contains("Rows") && names.contains("Pool"),
        "{names:?}"
    );
    assert!(!at_the_alias.is_empty());
    for source in used {
        let found = whole_row_findings(declare, source, &names);

        assert!(!found.is_empty(), "{source}");
    }
}

#[test]
fn the_read_inside_get_is_kept() {
    let names = whole_row_names(&[]);
    let the_read = "impl Commissions<'_> { #[use_case] pub async fn get(&self, #[ports] ports: &Ports, query: Query) -> CommissionResult<Output> { let stored = ports.commission_rows.find(&query.commission_id).await?.ok_or_else(gone)?; } }";
    let not_the_port = [
        "/// The commission_rows field and CommissionRows.\nfn list() {}",
        "fn word() -> &'static str { \"commission_rows CommissionRows\" }",
        "fn first(rows: Vec<CommissionRow>) { rows.iter().find(|row| row.archived); }",
    ];

    let at_get = whole_row_findings(GET_FILE, the_read, &names);
    let real_get = whole_row_findings(GET_FILE, &read(&crates().join(GET_FILE)), &names);

    assert_eq!(at_get, Vec::<String>::new());
    assert_eq!(real_get, Vec::<String>::new());
    for source in not_the_port {
        let found = whole_row_findings(
            "application/src/commission/seats/declare.rs",
            source,
            &names,
        );

        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}

// The route table.

/// The commissions router's file and its building function.
const ROUTER_FILE: &str = "api/src/routes/commissions/mod.rs";
const ROUTER: &str = "commissions_router";

/// The path segment naming the commission's id.
const COMMISSION_ID: &str = "{id}";

/// The one commissions route naming no commission id: the create.
const CREATE_METHOD: &str = "POST";
const CREATE_PATH: &str = "/commissions";

/// axum's method routers, by their function and method names.
const METHOD_ROUTERS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "head", "options", "trace",
];

/// The calls on a method router that add no method.
const METHOD_LAYERS: [&str; 2] = ["layer", "route_layer"];

/// A route the router registers: its method, upper case, and its path as the router writes it.
type Route = (String, String);

fn free_fn<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemFn> {
    file.items.iter().find_map(|item| match item {
        Item::Fn(function) if function.sig.ident == name => Some(function),
        _ => None,
    })
}

/// The expression `function` returns as its body's tail.
fn tail_of(function: &ItemFn) -> Result<&Expr, String> {
    let Some(Stmt::Expr(tail, None)) = function.block.stmts.last() else {
        return Err(format!("fn {} ends in no expression", function.sig.ident));
    };
    Ok(tail)
}

fn unread(expr: &Expr) -> String {
    format!("an unread router shape: {}", expr.to_token_stream())
}

/// The one name `expr` is, as a path of one segment.
fn single_name(expr: &Expr) -> Option<String> {
    let Expr::Path(path) = expr else {
        return None;
    };
    let ident = path.path.get_ident().filter(|_| path.qself.is_none())?;
    Some(ident.to_string())
}

/// Whether `expr` is the path `segments`.
fn path_is(expr: &Expr, segments: &[&str]) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    let names: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    names == segments
}

/// The methods the method router `expr` serves; a call of a function of `file` with no
/// arguments is read through its body's tail.
fn methods_of(expr: &Expr, file: &syn::File) -> Result<Vec<String>, String> {
    match expr {
        Expr::Call(call) => {
            let name = single_name(&call.func).ok_or_else(|| unread(expr))?;
            if METHOD_ROUTERS.contains(&name.as_str()) {
                return Ok(vec![name.to_uppercase()]);
            }
            let helper = free_fn(file, &name)
                .filter(|_| call.args.is_empty())
                .ok_or_else(|| unread(expr))?;
            methods_of(tail_of(helper)?, file)
        }
        Expr::MethodCall(call) => {
            let mut methods = methods_of(&call.receiver, file)?;
            let name = call.method.to_string();
            if METHOD_ROUTERS.contains(&name.as_str()) {
                methods.push(name.to_uppercase());
            } else if !METHOD_LAYERS.contains(&name.as_str()) {
                return Err(unread(expr));
            }
            Ok(methods)
        }
        Expr::Paren(inner) => methods_of(&inner.expr, file),
        _ => Err(unread(expr)),
    }
}

/// Adds each route the router chain `expr` registers to `out`: from `Router::new()`, only
/// `.route(path, method router)` calls.
fn chain_routes(expr: &Expr, file: &syn::File, out: &mut BTreeSet<Route>) -> Result<(), String> {
    match expr {
        Expr::Call(call) if path_is(&call.func, &["Router", "new"]) && call.args.is_empty() => {
            Ok(())
        }
        Expr::MethodCall(call) if call.method == "route" => {
            chain_routes(&call.receiver, file, out)?;
            let arguments: Vec<&Expr> = call.args.iter().collect();
            let [Expr::Lit(path), method_router] = arguments.as_slice() else {
                return Err(unread(expr));
            };
            let Lit::Str(path) = &path.lit else {
                return Err(unread(expr));
            };
            for method in methods_of(method_router, file)? {
                out.insert((method, path.value()));
            }
            Ok(())
        }
        _ => Err(unread(expr)),
    }
}

/// Every route [`ROUTER`] registers in `source`.
fn router_routes(source: &str) -> Result<BTreeSet<Route>, String> {
    let file = parsed(ROUTER_FILE, source);
    let router = free_fn(&file, ROUTER).ok_or_else(|| format!("no fn {ROUTER}"))?;
    let mut routes = BTreeSet::new();
    chain_routes(tail_of(router)?, &file, &mut routes)?;
    Ok(routes)
}

fn names_commission_id(path: &str) -> bool {
    path.split('/').any(|segment| segment == COMMISSION_ID)
}

/// Whether `path` is an instance of `route`: segment for segment equal, a `{…}` segment
/// standing for any.
fn instance_of(path: &str, route: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    let template: Vec<&str> = route.split('/').collect();
    let same_shape = segments.len() == template.len();
    same_shape
        && segments.iter().zip(&template).all(|(segment, part)| {
            let placeholder = part.starts_with('{') && part.ends_with('}');
            (placeholder && !segment.is_empty()) || segment == part
        })
}

/// What breaks the match of the router's `routes` and the call table `calls`.
fn route_table_findings(routes: &BTreeSet<Route>, calls: &[CommissionCall]) -> Vec<String> {
    let (with_id, without_id): (BTreeSet<Route>, BTreeSet<Route>) = routes
        .iter()
        .cloned()
        .partition(|(_, path)| names_commission_id(path));
    let create = BTreeSet::from([(CREATE_METHOD.to_owned(), CREATE_PATH.to_owned())]);
    let table: Vec<Route> = calls
        .iter()
        .map(|call| (call.method.as_str().to_owned(), call.route.to_owned()))
        .collect();
    let rows: BTreeSet<Route> = table.iter().cloned().collect();
    let mut out = Vec::new();
    if without_id != create {
        out.push(format!(
            "the routes naming no commission id: {without_id:?}"
        ));
    }
    for (method, path) in with_id.difference(&rows) {
        out.push(format!("{method} {path} has no row in the call table"));
    }
    for (method, path) in rows.difference(&with_id) {
        out.push(format!(
            "the call table's {method} {path} is no route naming {COMMISSION_ID}"
        ));
    }
    if rows.len() != table.len() {
        out.push("the call table holds a route twice".to_owned());
    }
    let made_up = absent_commission();
    for call in calls {
        let path = call.path(made_up);
        if !instance_of(&path, call.route) {
            out.push(format!("{}: {path} is not on {}", call.label, call.route));
        }
    }
    out
}

/// The call table, built for a made-up column.
fn call_table() -> Vec<CommissionCall> {
    let column = domain::elements::workflow::ColumnId::from(uuid::Uuid::from_u128(0x5eed));
    every_commission_call(column)
}

fn router_source() -> String {
    read(&crates().join(ROUTER_FILE))
}

#[test]
fn every_commission_route_has_a_not_found_row() {
    let routes = router_routes(&router_source()).expect("the router reads");
    let table = call_table();

    let found = route_table_findings(&routes, &table);

    assert_eq!(found, Vec::<String>::new());
    let channel =
        ["PUT", "DELETE"].map(|method| (method.to_owned(), "/commissions/{id}/channel".to_owned()));
    for route in channel {
        assert!(routes.contains(&route), "{route:?}");
    }
}

#[test]
fn a_route_without_its_row_or_a_row_without_its_route_is_refused() {
    let routes = router_routes(&router_source()).expect("the router reads");
    let table = call_table();
    let extra_route = router_source().replacen(
        "Router::new()",
        "Router::new().route(\"/commissions/{id}/extra\", get(extra::read))",
        1,
    );
    let second_without_id = router_source().replacen(
        "Router::new()",
        "Router::new().route(\"/commissions/search\", get(search::find))",
        1,
    );
    let without_markup: Vec<CommissionCall> = table
        .iter()
        .filter(|call| call.label != "mark up a file")
        .cloned()
        .collect();
    let off_its_route: Vec<CommissionCall> = table
        .iter()
        .cloned()
        .map(|mut call| {
            if call.label == "mark up a file" {
                call.tail = "/files".to_owned();
            }
            call
        })
        .collect();

    let with_extra = route_table_findings(&router_routes(&extra_route).expect("reads"), &table);
    let with_search =
        route_table_findings(&router_routes(&second_without_id).expect("reads"), &table);
    let missing_markup = route_table_findings(&routes, &without_markup);
    let off_route = route_table_findings(&routes, &off_its_route);

    let expected_extra = ["GET /commissions/{id}/extra has no row in the call table".to_owned()];
    let expected_markup =
        ["POST /commissions/{id}/files/{file_id}/markup has no row in the call table".to_owned()];
    assert_eq!(with_extra, expected_extra);
    assert_eq!(missing_markup, expected_markup);
    assert_eq!(with_search.len(), 1, "{with_search:#?}");
    assert_eq!(off_route.len(), 1, "{off_route:#?}");
}

#[test]
fn a_router_shape_the_scan_cannot_read_is_refused() {
    let unreadable = [
        ("Router::new()", "Router::new().nest(\"/more\", more())"),
        (
            "Router::new()",
            "Router::new().route(\"/commissions/{id}/any\", any(every::method))",
        ),
        (
            "Router::new()",
            "Router::new().route(\"/commissions/{id}/on\", get(x).on(MethodFilter::POST, y))",
        ),
        ("Router::new()", "Router::new().route(PATH, get(x))"),
        (
            "Router::new()",
            "Router::new().route(\"/commissions/{id}/q\", axum::routing::get(x))",
        ),
        ("channel_methods()", "channel_methods(true)"),
    ];

    for (from, to) in unreadable {
        let source = router_source().replacen(from, to, 1);

        let routes = router_routes(&source);

        assert!(routes.is_err(), "{to}: {routes:?}");
    }
}

// Test-support's raw reach.

/// adapter-pg's generated queries module, and its commission module.
const RAW_QUERIES: &str = "queries";
const RAW_COMMISSION: &str = "commission";

/// What a raw reach is shown as.
const RAW_REACH: &str = "adapter_pg::queries::commission";

/// The den world's file, under test-support's `src`, and its seeders, each reaching the raw
/// commission queries exactly once.
const SEEDERS_FILE: &str = "pg_app/den_world.rs";
const SEEDERS: [(&str, usize); 2] = [("seed", 1), ("DenWorld::add_bob_to_old_sketch", 1)];

fn test_support_src() -> PathBuf {
    crates().join("test-support/src")
}

/// Every Rust file under `folder`, test files included, sorted.
fn every_rust_file(folder: &Path) -> Vec<PathBuf> {
    let entries = std::fs::read_dir(folder).expect("a readable folder");
    let mut files: Vec<PathBuf> = entries
        .map(|entry| entry.expect("a folder entry").path())
        .flat_map(|path| {
            if path.is_dir() {
                every_rust_file(&path)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect();
    files.sort();
    files
}

fn tokens_of(file: &str, source: &str) -> TokenStream {
    parsed(file, source).to_token_stream()
}

/// The entries of a use group's `tokens`, split at its top-level commas.
fn use_entries(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut entries = vec![Vec::new()];
    for tree in tokens {
        let comma = matches!(&tree, TokenTree::Punct(punct) if punct.as_char() == ',');
        if comma {
            entries.push(Vec::new());
        } else if let Some(entry) = entries.last_mut() {
            entry.push(tree);
        }
    }
    entries.retain(|entry| !entry.is_empty());
    entries
}

/// The brace group after a `name::` starting at `at`, when `name` is one of `names`.
fn group_after(level: &[TokenTree], at: usize, names: &BTreeSet<String>) -> Option<TokenStream> {
    let named = ident_at(level, at).is_some_and(|name| names.contains(&name));
    if !named || !path_separator_at(level, at + 1) {
        return None;
    }
    match level.get(at + 3) {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
            Some(group.stream())
        }
        _ => None,
    }
}

/// The names bound at `at` to one of `names`: `name as alias`, or `name::{self as alias}`.
fn renames_at(level: &[TokenTree], at: usize, names: &BTreeSet<String>) -> Vec<String> {
    let renamed = ident_at(level, at).is_some_and(|name| names.contains(&name))
        && ident_at(level, at + 1).as_deref() == Some("as");
    let mut out: Vec<String> = ident_at(level, at + 2)
        .filter(|alias| renamed && alias != "_")
        .into_iter()
        .collect();
    let in_group = group_after(level, at, names)
        .map(use_entries)
        .unwrap_or_default();
    for entry in in_group {
        let self_renamed = ident_at(&entry, 0).as_deref() == Some("self")
            && ident_at(&entry, 1).as_deref() == Some("as");
        if let Some(alias) = ident_at(&entry, 2).filter(|alias| self_renamed && alias != "_") {
            out.push(alias);
        }
    }
    out
}

/// The generated queries module's name and every name `sources` bind to it, at any depth,
/// through any chain.
fn queries_names(sources: &[TokenStream]) -> BTreeSet<String> {
    let all_levels: Vec<Vec<TokenTree>> = sources.iter().cloned().flat_map(levels).collect();
    let mut names = BTreeSet::from([RAW_QUERIES.to_owned()]);
    loop {
        let mut grown = names.clone();
        for level in &all_levels {
            for at in 0..level.len() {
                grown.extend(renames_at(level, at, &names));
            }
        }
        if grown == names {
            return names;
        }
        names = grown;
    }
}

/// How many raw reaches start at `at`: `queries::commission`, `queries::*`, and each
/// `commission` or `*` entry of a `queries::{…}` group, through any of `names`.
fn raw_reaches_at(level: &[TokenTree], at: usize, names: &BTreeSet<String>) -> usize {
    let named = ident_at(level, at).is_some_and(|name| names.contains(&name));
    if !named || !path_separator_at(level, at + 1) {
        return 0;
    }
    let next = at + 3;
    if ident_at(level, next).as_deref() == Some(RAW_COMMISSION) || punct_at(level, next, '*') {
        return 1;
    }
    let entries = group_after(level, at, names)
        .map(use_entries)
        .unwrap_or_default();
    entries
        .iter()
        .filter(|entry| {
            ident_at(entry, 0).as_deref() == Some(RAW_COMMISSION) || punct_at(entry, 0, '*')
        })
        .count()
}

/// Each raw reach in `source`, the file at `file` under test-support's `src`, past the den
/// world's seeders' allowance.
fn raw_findings(file: &str, source: &str, names: &BTreeSet<String>) -> Vec<String> {
    let found: Vec<Naming> = scopes(&parsed(file, source))
        .into_iter()
        .flat_map(|Scope { context, tokens }| {
            let reaches: usize = levels(tokens)
                .iter()
                .map(|level| {
                    (0..level.len())
                        .map(|at| raw_reaches_at(level, at, names))
                        .sum::<usize>()
                })
                .sum();
            std::iter::repeat_n((context, RAW_REACH.to_owned()), reaches)
        })
        .collect();
    let allowed: Vec<(&str, &str, usize)> = if file == SEEDERS_FILE {
        SEEDERS
            .iter()
            .map(|(seeder, count)| (*seeder, RAW_REACH, *count))
            .collect()
    } else {
        Vec::new()
    };
    past_allowance(found, &allowed)
        .into_iter()
        .map(|(scope, name)| format!("{file}: `{name}` in {scope}"))
        .collect()
}

/// Each raw reach in the Rust files under `src`, aliases made in any of them included.
fn raw_tree_findings(src: &Path) -> Vec<String> {
    let files: Vec<(String, String)> = every_rust_file(src)
        .iter()
        .map(|path| {
            let inside = path.strip_prefix(src).expect("a file under src");
            let file = inside.to_string_lossy().replace('\\', "/");
            (file, read(path))
        })
        .collect();
    let sources: Vec<TokenStream> = files
        .iter()
        .map(|(file, source)| tokens_of(file, source))
        .collect();
    let names = queries_names(&sources);
    files
        .iter()
        .flat_map(|(file, source)| raw_findings(file, source, &names))
        .collect()
}

/// How many raw reaches the scope `context` of `source` holds.
fn reaches_in(file: &str, source: &str, context: &str) -> usize {
    let names = queries_names(&[tokens_of(file, source)]);
    scopes(&parsed(file, source))
        .into_iter()
        .filter(|scope| scope.context == context)
        .flat_map(|scope| levels(scope.tokens))
        .map(|level| {
            (0..level.len())
                .map(|at| raw_reaches_at(&level, at, &names))
                .sum::<usize>()
        })
        .sum()
}

#[test]
fn the_raw_commission_queries_are_named_only_in_den_world_s_seeders() {
    let src = test_support_src();

    let offenders = raw_tree_findings(&src);

    assert_eq!(offenders, Vec::<String>::new());
    let den_world = read(&src.join(SEEDERS_FILE));
    for (seeder, count) in SEEDERS {
        let reached = reaches_in(SEEDERS_FILE, &den_world, seeder);

        assert_eq!(reached, count, "{seeder}");
    }
}

#[test]
fn the_raw_reach_scan_refuses_each_spelling_and_site() {
    let call = "add_participant(pool, id, &did, Utc::now()).await;";
    let refused = [
        (
            "pg_app/served.rs".to_owned(),
            format!("async fn join(pool: &PgPool) {{ adapter_pg::queries::commission::{call} }}"),
        ),
        (
            SEEDERS_FILE.to_owned(),
            format!(
                "impl DenWorld {{ pub async fn archived_or_live(&self) {{ adapter_pg::queries::commission::{call} }} }}"
            ),
        ),
        (
            SEEDERS_FILE.to_owned(),
            format!(
                "async fn seed() {{ adapter_pg::queries::commission::{call} adapter_pg::queries::commission::{call} }}"
            ),
        ),
        (
            SEEDERS_FILE.to_owned(),
            format!(
                "mod inner {{ async fn seed() {{ adapter_pg::queries::commission::{call} }} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "use adapter_pg::queries::commission;\nasync fn join() {{ commission::{call} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "use adapter_pg::queries::{{account, commission as c}};\nasync fn join() {{ c::{call} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!("use adapter_pg::queries;\nasync fn join() {{ queries::commission::{call} }}"),
        ),
        (
            "seeder.rs".to_owned(),
            format!("use adapter_pg::queries::*;\nasync fn join() {{ commission::{call} }}"),
        ),
        (
            "seeder.rs".to_owned(),
            format!("use adapter_pg::queries::commission::*;\nasync fn join() {{ {call} }}"),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "use adapter_pg::{{queries::{{commission::add_participant}}}};\nasync fn join() {{ {call} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "async fn join() {{ use adapter_pg::queries as raw; raw::commission::{call} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "use adapter_pg::{{queries::{{self as raw}}}};\nasync fn join() {{ raw::commission::{call} }}"
            ),
        ),
        (
            "seeder.rs".to_owned(),
            format!("async fn join() {{ adapter_pg::r#queries::r#commission::{call} }}"),
        ),
        (
            "seeder.rs".to_owned(),
            "fn joiner() { let join = adapter_pg::queries::commission::add_participant; }"
                .to_owned(),
        ),
        (
            "seeder.rs".to_owned(),
            format!(
                "macro_rules! join {{ ($pool:expr) => {{ adapter_pg::queries::commission::{call} }}; }}"
            ),
        ),
    ];

    for (file, source) in refused {
        let names = queries_names(&[tokens_of(&file, &source)]);

        let found = raw_findings(&file, &source, &names);

        assert!(!found.is_empty(), "{file}: {source}");
    }
}

#[test]
fn the_seeders_reach_and_other_queries_are_kept() {
    let names = BTreeSet::from([RAW_QUERIES.to_owned()]);
    let kept = [
        (
            SEEDERS_FILE,
            "async fn seed(pool: &PgPool) { adapter_pg::queries::commission::add_participant(pool).await; }\nimpl DenWorld { pub async fn add_bob_to_old_sketch(&self) { adapter_pg::queries::commission::add_participant(pool).await; } }",
        ),
        (
            "pg_app/den_world/identity.rs",
            "async fn deactivate(pool: &PgPool) { adapter_pg::queries::account::soft_delete(pool).await; }",
        ),
        (
            "pg_app/statements.rs",
            "fn listed() { let queries = Path::new(\"adapter-pg/queries\"); let commission = queries.join(\"commission\"); }",
        ),
        (
            "seeder.rs",
            "use adapter_pg::queries::{account};\nfn word() -> &'static str { \"queries::commission\" }",
        ),
    ];

    for (file, source) in kept {
        let found = raw_findings(file, source, &names);

        assert_eq!(found, Vec::<String>::new(), "{file}: {source}");
    }
}

#[test]
fn a_raw_reach_in_any_folder_is_found_and_an_alias_made_elsewhere_followed() {
    let src = fixture_tree(
        "raw_reach",
        &[
            (
                "lib.rs",
                "pub mod pg_app;\npub use adapter_pg::queries as raw;",
            ),
            ("pg_app.rs", "mod deep;"),
            (
                "pg_app/deep.rs",
                "async fn join(pool: &PgPool) { crate::raw::commission::add_participant(pool).await; }",
            ),
        ],
    );

    let found = raw_tree_findings(&src);

    let expected = ["pg_app/deep.rs: `adapter_pg::queries::commission` in join".to_owned()];
    assert_eq!(found, expected);
}
