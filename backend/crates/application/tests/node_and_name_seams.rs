//! No driver and no use case reaches the Den's node port but the den's write gate, which names
//! the node port's accessor once, and Atta's checked-name constructor is named only in
//! `names::display_name`. The scan reads the production sources of `application` and of every
//! driver crate cargo metadata names, token by token, macro bodies included, and matches every
//! identifier, so a call, a function reference, a qualified path and an alias all count.

mod seams;

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use proc_macro2::TokenStream;
use quote::ToTokens;
use seams::{
    NODE_PORT, fixture_tree, identifiers, item_name, named_in_free_fn, walk, with_aliases,
    workspace_sources,
};

/// Atta's checked-name constructor, named only in [`DISPLAY_NAME`] in the names file.
const HOST_CHECKED: &str = "host_checked";

/// The one free function, in the names file, that may name [`HOST_CHECKED`].
const DISPLAY_NAME: &str = "display_name";

/// The unit of work's node-port accessor, named only once, in [`GATE`] in the gate file.
const NODES: &str = "nodes";

/// The one free function, in the gate file, that may name [`NODES`].
const GATE: &str = "gate";

/// The node port's type names, which the gate file alone may name, anywhere in it.
const NODE_PORT_TYPES: [&str; 3] = ["NodeRepo", "NodeReads", "NodeWrites"];

fn crates() -> PathBuf {
    seams::crates()
}

/// The one file whose [`DISPLAY_NAME`] may name [`HOST_CHECKED`].
fn names_file() -> PathBuf {
    crates().join("application/src/den/names.rs")
}

/// The one file whose [`GATE`] may name [`NODES`], and that may name [`NODE_PORT_TYPES`].
fn gate_file() -> PathBuf {
    crates().join("application/src/den/gate.rs")
}

/// Which exemption a source file holds, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seat {
    /// None.
    Elsewhere,
    /// The names file: [`DISPLAY_NAME`] may name [`HOST_CHECKED`] once.
    Names,
    /// The gate file: [`GATE`] may name [`NODES`] once, and any item [`NODE_PORT_TYPES`].
    Gate,
}

/// The seat of the file at `path`.
fn seat_of(path: &Path) -> Seat {
    if path == names_file() {
        return Seat::Names;
    }
    if path == gate_file() {
        return Seat::Gate;
    }
    Seat::Elsewhere
}

/// The forbidden identifiers: the node port's, [`HOST_CHECKED`], and every name `sources` bind
/// to one of them, through any chain of aliases.
fn forbidden_names(sources: &[String]) -> BTreeSet<String> {
    let seeds: Vec<&str> = NODE_PORT.iter().copied().chain([HOST_CHECKED]).collect();
    with_aliases(&seeds, sources)
}

/// Each forbidden identifier in `source`, with the item it sits in. In the names file, one
/// [`HOST_CHECKED`] inside [`DISPLAY_NAME`] passes; in the gate file, one [`NODES`] inside
/// [`GATE`] passes, and so does every [`NODE_PORT_TYPES`] name.
fn findings(source: &str, forbidden: &BTreeSet<String>, seat: Seat) -> Vec<String> {
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
        let (exempt, mut allowed) = match seat {
            Seat::Elsewhere => ("", 0),
            Seat::Names => (
                HOST_CHECKED,
                named_in_free_fn(item, DISPLAY_NAME, HOST_CHECKED).min(1),
            ),
            Seat::Gate => (NODES, named_in_free_fn(item, GATE, NODES).min(1)),
        };
        let summary = item_name(item);
        for ident in identifiers(item.to_token_stream()) {
            if !forbidden.contains(&ident) {
                continue;
            }
            if seat == Seat::Gate && NODE_PORT_TYPES.contains(&ident.as_str()) {
                continue;
            }
            if ident == exempt && allowed > 0 {
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
    walk(src)
        .production
        .into_iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(&path).expect("a readable source");
            findings(&source, forbidden, seat_of(&path))
                .into_iter()
                .map(move |finding| format!("{}: {finding}", path.display()))
        })
        .collect()
}

/// The `src` of each crate the scan reads: the drivers, as cargo metadata names them, and
/// the use-case layer.
fn scanned_srcs() -> Vec<PathBuf> {
    seams::node_port_scanned()
        .iter()
        .map(|member| member.folder.join("src"))
        .collect()
}

#[test]
fn the_scan_reads_every_scanned_crate_and_the_exempt_files() {
    for src in scanned_srcs() {
        let production = walk(&src).production;

        assert!(!production.is_empty(), "{} has no sources", src.display());
    }
    let application = walk(&crates().join("application/src")).production;
    assert!(application.contains(&names_file()));
    assert!(application.contains(&gate_file()));
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
fn display_name_names_the_checked_name_once() {
    let source = std::fs::read_to_string(names_file()).expect("names.rs reads");
    let file = syn::parse_file(&source).expect("names.rs parses");

    let named: usize = file
        .items
        .iter()
        .map(|item| named_in_free_fn(item, DISPLAY_NAME, HOST_CHECKED))
        .sum();

    assert_eq!(named, 1);
}

#[test]
fn the_gate_names_the_node_port_once() {
    let source = std::fs::read_to_string(gate_file()).expect("gate.rs reads");
    let file = syn::parse_file(&source).expect("gate.rs parses");
    let forbidden = plain_forbidden();

    let in_the_gate: usize = file
        .items
        .iter()
        .map(|item| named_in_free_fn(item, GATE, NODES))
        .sum();
    let in_the_file = identifiers(file.to_token_stream())
        .iter()
        .filter(|ident| *ident == NODES)
        .count();

    assert_eq!(in_the_gate, 1);
    assert_eq!(in_the_file, 1);
    let elsewhere_in_the_gate_file = [
        "fn other(uow: Unit<'_>) { let rows = uow.nodes(); }",
        "async fn gate(uow: Unit<'_>) { let rows = uow.nodes(); let more = uow.nodes(); }",
        "impl Host for DenWriteHost<'_> { async fn gate(&mut self) { self.unit.nodes(); } }",
        "mod inner { async fn gate(uow: Unit<'_>) { uow.nodes(); } }",
        "fn reach(pool: PgPool) { let store = adapter_pg::PgNodeStore::new(pool); }",
        "fn reach(store: Arc<dyn NodeStore>) {}",
    ];
    for source in elsewhere_in_the_gate_file {
        assert!(
            !findings(source, &forbidden, Seat::Gate).is_empty(),
            "{source}"
        );
    }
    let the_gate = "async fn gate(uow: Unit<'_>) { let rows = uow.nodes(); write(&mut *rows); }\nasync fn write(rows: &mut dyn NodeRepo) {}";
    assert_eq!(
        findings(the_gate, &forbidden, Seat::Gate),
        Vec::<String>::new()
    );
    let src = fixture_tree(
        "gate_elsewhere",
        &[
            ("lib.rs", "mod den;"),
            ("den.rs", "mod write;\nmod gate;"),
            (
                "den/write.rs",
                "async fn gate(uow: Unit<'_>) { let rows = uow.nodes(); }\nfn rows(repo: &mut dyn NodeRepo) {}",
            ),
        ],
    );
    let in_write = tree_findings(&src, &forbidden);
    assert_eq!(in_write.len(), 2, "{in_write:#?}");
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
        "async fn reach(backend: MemBackend) -> anyhow::Result<()> { let den_store = backend.node_store(); let _read = den_store.children(&domain::elements::node::NodeId::mint()).await?; Ok(()) }",
        "fn reach() { let store = adapter_mem::MemNodeStore::default(); }",
        "fn reach(writes: adapter_mem::MemNodeWrites<'_>) {}",
    ];

    for source in planted {
        assert!(
            !findings(source, &forbidden, Seat::Elsewhere).is_empty(),
            "{source}"
        );
        assert!(
            !findings(source, &forbidden, Seat::Names).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn the_scan_refuses_the_checked_name_outside_display_name() {
    let forbidden = plain_forbidden();
    let elsewhere = [
        "fn name(text: String) -> DisplayName { DisplayName::host_checked(text) }",
        "fn names(texts: Vec<String>) { texts.into_iter().map(atta::DisplayName::host_checked); }",
        "use atta::DisplayName as Shown; fn name(text: String) -> Shown { Shown::host_checked(text) }",
        "fn names(text: String) { let names = vec![atta::DisplayName::host_checked(text)]; }",
        "fn display_name(name: &NodeName) -> DisplayName { DisplayName::host_checked(name.to_string()) }",
    ];
    let in_the_names_file = [
        "fn found(text: String) -> DisplayName { DisplayName::host_checked(text) }",
        "impl<'a> DenHost<'a> { fn display_name(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
        "impl Other { fn display_name(&self, text: String) -> DisplayName { DisplayName::host_checked(text) } }",
        "fn display_name(text: String) -> DisplayName { DisplayName::host_checked(text.clone()); DisplayName::host_checked(text) }",
        "mod inner { fn display_name(text: String) -> DisplayName { DisplayName::host_checked(text) } }",
    ];

    for source in elsewhere {
        assert!(
            !findings(source, &forbidden, Seat::Elsewhere).is_empty(),
            "{source}"
        );
        assert!(
            !findings(source, &forbidden, Seat::Gate).is_empty(),
            "{source}"
        );
    }
    for source in in_the_names_file {
        assert!(
            !findings(source, &forbidden, Seat::Names).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn display_name_may_name_the_checked_name_but_not_the_node_port() {
    let forbidden = plain_forbidden();
    let allowed = "fn display_name(name: &NodeName) -> DisplayName { DisplayName::host_checked(name.to_string()) }";
    let reaching = "fn display_name(uow: Unit<'_>, name: &NodeName) -> DisplayName { uow.nodes(); DisplayName::host_checked(name.to_string()) }";

    let allowed_findings = findings(allowed, &forbidden, Seat::Names);
    let reaching_findings = findings(reaching, &forbidden, Seat::Names);

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
        let found = findings(source, &forbidden, Seat::Elsewhere);
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
        assert!(
            !findings(source, &forbidden, Seat::Elsewhere).is_empty(),
            "{source}"
        );
    }
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

#[test]
fn a_module_declared_under_cfg_test_is_skipped_with_its_children() {
    let reaching = "fn reach(uow: Unit<'_>) { uow.nodes(); }";
    let src = fixture_tree(
        "test_only_modules",
        &[
            (
                "lib.rs",
                "mod den;\n#[cfg(test)]\nmod test_world;\nmod world;",
            ),
            ("test_world.rs", "mod seeds;"),
            ("test_world/seeds.rs", reaching),
            ("world.rs", reaching),
            ("den.rs", "#[cfg(test)]\nmod probe;\nmod write;"),
            ("den/probe.rs", reaching),
            ("den/write.rs", "#[cfg(test)]\nmod log_tests;"),
            ("den/write/log_tests.rs", reaching),
        ],
    );

    let production = walk(&src).production;
    let found = tree_findings(&src, &plain_forbidden());

    let expected = ["den/write.rs", "den.rs", "lib.rs", "world.rs"].map(|path| src.join(path));
    assert_eq!(production, expected);
    assert_eq!(found.len(), 1, "{found:#?}");
}
