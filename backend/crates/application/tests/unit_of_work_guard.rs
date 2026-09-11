//! CI guard for the balance of a unit of work (DD `24150017`, DD `55836674`).
//!
//! The type system already makes the *common* mistakes unrepresentable: a write
//! is only reachable on the transaction-bound [`UnitOfWork`] handle, and
//! [`commit`](UnitOfWork::commit) consumes that handle so it cannot be reused.
//! This test closes the two residual holes the type system cannot see, because
//! Rust has no linear types and so cannot demand that a value be consumed:
//!
//! 1. **A unit that is opened and never committed.** Dropping the handle rolls
//!    the whole unit back — silently, on a background executor. A use case that
//!    issues its writes and then returns `Ok` without committing therefore
//!    reports success while persisting nothing. This shipped five times at once
//!    (`commission::create`, `commission::place`,
//!    `commission::invitations::revoke`, `account::invitation::issue`,
//!    `commission::view::grant`) and was invisible until the api suite could run
//!    again.
//! 2. **Two units opened in one function.** The second `let` shadows the first,
//!    so the first's writes are abandoned while the second commits — and both
//!    hold a pool connection for the request. `commission::view::grant` did
//!    exactly this: it provisioned the grantee on one unit and granted on
//!    another, so the grant landed and the grantee did not.
//!
//! Neither is detectable by type, and neither is reliably caught by a runtime
//! `Drop` hook: dropping *is* the documented rollback path, so a hook cannot
//! tell a deliberate error-path abandonment from a forgotten commit. A source
//! rule can, because it judges the shape of the function rather than one
//! execution of it.
//!
//! **Scope.** This guard reads `application/src/**` only. The application layer
//! owns the transaction (DD `55836674`): no driver and no background job opens
//! one, so a `begin` anywhere else is a separate violation with a separate home.
//!
//! **Limits, stated plainly.** This is a *shape* rule, not a path analysis. A
//! function that commits on one branch and not another satisfies it. That case
//! belongs to the runtime `Drop` tracing, which is the complement to this guard,
//! not its substitute.
//!
//! [`UnitOfWork`]: domain::ports::UnitOfWork

use std::path::{Path, PathBuf};

/// Opening a unit: the factory call on the neutral [`Database`] port, in its
/// canonical whitespace-free form. The receiver varies (`self.ports()`, a bound
/// `ports`, a field), so only the call itself is matched.
///
/// [`Database`]: domain::ports::Database
const OPEN: &str = ".begin()";

/// Closing a unit. `rollback` counts too: abandoning a unit *explicitly* is a
/// deliberate act the reader can see, which is the whole point — it is the
/// silent drop this guard is here to catch.
const CLOSE: [&str; 2] = [".commit()", ".rollback()"];

/// Functions allowed to open a unit without closing it, keyed by
/// `<crate-relative path>::<fn name>`.
///
/// Empty, and it should stay that way. A use case that opens a unit it does not
/// close is either a bug or a new pattern that needs a home — in both cases a
/// design question for the Engineer, not a quiet edit to this list. If one ever
/// lands here it carries its reason inline, as the sibling guard's exemptions
/// do (`adapter-pg/tests/it/no_bare_pool_writes.rs`).
const EXEMPT: &[&str] = &[];

/// The application sources this guard scans, relative to this crate's
/// `CARGO_MANIFEST_DIR` (`backend/crates/application`).
fn scanned_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `*.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// Whitespace-stripped form of a source chunk — the canonical matching space,
/// so a call broken across lines by rustfmt reads the same as an inline one.
fn collapsed(chunk: &str) -> String {
    chunk.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

/// Does this line begin a function definition? Matched on the *trimmed* line, so
/// a `///` doc comment or a `//` note mentioning `fn` can never open one.
fn starts_function(line: &str) -> bool {
    let trimmed = line.trim_start();
    [
        "pub async fn ",
        "pub fn ",
        "async fn ",
        "fn ",
        "pub(crate) fn ",
    ]
    .iter()
    .any(|kw| trimmed.starts_with(kw))
}

/// The function name on a definition line — the text between `fn ` and the
/// first `(` or `<`. Falls back to `"?"` rather than panicking, so a shape this
/// guard has not seen degrades the *message*, never the verdict.
fn function_name(line: &str) -> String {
    let after = match line.split_once("fn ") {
        Some((_, rest)) => rest,
        None => return "?".to_string(),
    };
    after
        .split(['(', '<', ' '])
        .next()
        .unwrap_or("?")
        .to_string()
}

/// Every `(name, body)` pair in `src`, the body brace-matched from the
/// signature's opening `{` to its close.
///
/// Bodies nest: a function defined inside another appears both on its own and
/// within its parent's body. That is deliberate — a unit opened by an inner
/// function is still opened on the outer function's watch.
fn functions(src: &str) -> Vec<(String, String)> {
    let bytes: Vec<char> = src.chars().collect();
    let mut out = Vec::new();

    for (offset, line) in line_offsets(src) {
        if !starts_function(line) {
            continue;
        }
        let Some(open) = bytes[offset..].iter().position(|&c| c == '{') else {
            continue;
        };
        let start = offset + open;
        let mut depth = 0usize;
        for (i, &c) in bytes[start..].iter().enumerate() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body: String = bytes[start..=start + i].iter().collect();
                        out.push((function_name(line), body));
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Each line of `src` with its character offset from the start.
fn line_offsets(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in src.lines() {
        out.push((offset, line));
        offset += line.chars().count() + 1;
    }
    out
}

/// How many units this body opens, and whether it closes any.
fn balance(body: &str) -> (usize, bool) {
    let collapsed = collapsed(body);
    let opened = collapsed.matches(OPEN).count();
    let closed = CLOSE.iter().any(|close| collapsed.contains(close));
    (opened, closed)
}

/// The `<crate>/src/<file…>` suffix of a scanned path — the key exemptions are
/// matched on, so an exemption cannot leak across crates.
fn crate_relative(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = canon.to_string_lossy().replace('\\', "/");
    match text.rsplit_once("/crates/") {
        Some((_, rel)) => rel.to_string(),
        None => text,
    }
}

/// Every unbalanced unit of work in `files`, as a reader-facing complaint.
fn offenders(files: &[PathBuf]) -> Vec<String> {
    let mut hits = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", file.display()));
        let relative = crate_relative(file);

        for (name, body) in functions(&src) {
            let key = format!("{relative}::{name}");
            if EXEMPT.contains(&key.as_str()) {
                continue;
            }
            let (opened, closed) = balance(&body);
            if opened == 0 {
                continue;
            }
            if opened > 1 {
                hits.push(format!(
                    "{key} opens {opened} units of work — the later `let` shadows the \
                     earlier, abandoning its writes while holding its connection"
                ));
            }
            if !closed {
                hits.push(format!(
                    "{key} opens a unit of work and never commits it — every write it \
                     issues rolls back on drop, silently, and the caller still sees Ok"
                ));
            }
        }
    }
    hits
}

#[test]
fn every_opened_unit_of_work_is_committed() {
    // Prove the detector fires, and does not over-fire, before trusting a green
    // scan: an exit status is not evidence, only an exercised assertion is.
    let committed = "{ let mut uow = self.ports().database.begin().await?; \
                     uow.commissions().create(&c).await?; uow.commit().await?; }";
    let abandoned = "{ let mut uow = self.ports().database.begin().await?; \
                      uow.commissions().create(&c).await?; }";
    let doubled = "{ let mut uow = db.begin().await?; let mut uow = db.begin().await?; \
                    uow.commit().await?; }";
    let read_only = "{ let found = self.ports().commissions.find(&id).await?; }";

    assert_eq!(
        balance(committed),
        (1, true),
        "the guard's detector misread a correctly committed unit"
    );
    assert_eq!(
        balance(abandoned),
        (1, false),
        "the guard's detector failed to flag a unit that is never committed"
    );
    assert_eq!(
        balance(doubled).0,
        2,
        "the guard's detector failed to count a second unit opened in one function"
    );
    assert_eq!(
        balance(read_only),
        (0, false),
        "the guard's detector flagged a read-only function — those open no unit"
    );

    // The body extractor must survive the shapes this crate actually writes: a
    // multi-line signature, and a `json!({ ... })` payload whose braces nest.
    let source = r#"
impl Commissions<'_> {
    /// A fn mentioned in a doc comment must not open a function.
    pub async fn set(
        &self,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let entry = json!({ "from": from, "to": to });
        let mut uow = self.ports().database.begin().await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
"#;
    let extracted = functions(source);
    assert_eq!(
        extracted.len(),
        1,
        "the body extractor found {} functions in a one-function source — a doc \
         comment or a nested brace is being misread",
        extracted.len()
    );
    assert_eq!(extracted[0].0, "set", "the extractor misnamed the function");
    assert_eq!(
        balance(&extracted[0].1),
        (1, true),
        "the extractor truncated a body around its `json!` braces"
    );

    // Now scan the real application layer.
    let files = rust_files(&scanned_src_dir());
    assert!(
        files.len() > 30,
        "guard scanned suspiciously few files ({}) — check the path",
        files.len()
    );

    let scanned_units: usize = files
        .iter()
        .map(|file| {
            let src = std::fs::read_to_string(file).expect("readable");
            functions(&src)
                .iter()
                .map(|(_, body)| balance(body).0)
                .sum::<usize>()
        })
        .sum();
    assert!(
        scanned_units > 20,
        "guard found only {scanned_units} units of work across the application layer \
         — the `{OPEN}` match looks hollowed out"
    );

    let offenders = offenders(&files);
    assert!(
        offenders.is_empty(),
        "unbalanced unit(s) of work:\n  {}\n\nA unit is closed by `commit()` (or an \
         explicit `rollback()`); dropping it rolls back in silence. See DD 24150017 \
         and the module note on this guard.",
        offenders.join("\n  ")
    );
}
