//! CI guard for the compile-enforced Unit of Work (DD `24150017`).
//!
//! The type system already makes the *common* mistake unrepresentable: a write
//! method is only reachable on the transaction-bound [`UnitOfWork`] handle, which
//! holds no pool, so a handler cannot issue `.execute(&pool)`. This test closes the
//! one residual hole the type system can't see — an *adapter author* re-introducing
//! a bare-pool write inside an adapter (where the pool is legitimately in scope for
//! reads). It scans the private-store write adapters and fails if a write executes
//! straight on the pool (`.execute(&self.pool)`) anywhere outside the two
//! documented exceptions.
//!
//! Why a grep and not clippy's `disallowed-methods`: that lint bans a method by
//! *path* (`Executor::execute`), so it cannot tell `.execute(&self.pool)` (banned)
//! from `.execute(&mut *conn)` (the correct on-transaction write) — it would reject
//! both. The receiver is the whole signal, so a textual check is the precise tool,
//! and it runs in the `cargo test` gate CI already invokes.
//!
//! **Reads on the pool are fine** and untouched: this bans only `.execute(...)` —
//! the write verb — on `&self.pool`. `fetch_*(&self.pool)` (reads) stays legal.

use std::path::{Path, PathBuf};

/// The bare-pool *write* signature this guard bans, in its canonical whitespace-free
/// form. A write that executes directly on the pool skips the Unit of Work; on a
/// transaction-bound view the receiver is `&mut *self.conn` instead, never
/// `&self.pool`. Matching is whitespace-insensitive (see [`is_bare_pool_write`]), so
/// `.execute( &self.pool )` is caught too.
const BANNED: &str = ".execute(&self.pool)";

/// Files allowed to keep a bare-pool write, keyed by **crate-relative path** — not
/// bare filename, so an exemption can't leak across crates. (Both `adapter-pg/src`
/// and `adapter-atproto/src` are scanned, and both have a `profile.rs`; a basename
/// key would silently exempt the atproto one too.) Each is a DOCUMENTED exception,
/// not a silent skip: it writes on the pool because it has **no transactional
/// invariant to uphold** — there is no domain unit of work to thread through:
///
/// - `adapter-pg/src/session_store.rs` — bound by the external
///   `tower_sessions_core::SessionStore` trait: fixed `&self`, each op independently
///   atomic, no place for our UnitOfWork.
/// - `adapter-atproto/src/auth_store.rs` — bound by the external
///   `jacquard_oauth::ClientAuthStore` trait, same shape as `session_store`.
/// - `adapter-pg/src/profile.rs` — the read-through profile **cache** fill
///   (`PgProfileCache::put`): a best-effort, single-statement upsert on the GET read
///   path whose failure the caller swallows. It is not a domain write, so routing it
///   through a write transaction would make a read endpoint open one for nothing
///   (Engineer's call).
///
/// If a *new* file needs to be exempted, that is a design question (does its write
/// truly have no transactional home?), not a quiet edit to this list.
const EXEMPT: &[&str] = &[
    "adapter-pg/src/session_store.rs",
    "adapter-atproto/src/auth_store.rs",
    "adapter-pg/src/profile.rs",
];

/// The private-store write adapters this guard scans, relative to this crate's
/// `CARGO_MANIFEST_DIR` (`backend/crates/adapter-pg`). `adapter-atproto` is included
/// because its `auth_store` persists OAuth state to Postgres on a pool, so it is a
/// pg-writing adapter even though it lives behind the public boundary.
fn scanned_src_dirs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    vec![
        manifest.join("src"),
        manifest.join("../adapter-atproto/src"),
    ]
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

/// Does this source line carry a bare-pool write? The single detection rule, kept
/// in one place so the guard and its self-check exercise the *same* logic.
///
/// Whitespace-insensitive: all ASCII whitespace is stripped before the match, so
/// `.execute( &self.pool )` collapses to the canonical [`BANNED`] form and is caught.
/// This still distinguishes `&self.pool` (banned) from `&mut *self.conn` (the correct
/// on-transaction write) — stripping whitespace from the latter yields
/// `.execute(&mut*self.conn)`, which does not contain [`BANNED`].
fn is_bare_pool_write(line: &str) -> bool {
    let collapsed: String = line.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    collapsed.contains(BANNED)
}

/// The `<crate>/src/<file…>` suffix of a scanned path — the key exemptions are
/// matched on, so `adapter-pg/src/profile.rs` (exempt) is never confused with
/// `adapter-atproto/src/profile.rs` (not exempt). Canonicalizes first to resolve the
/// `..` in the adapter-atproto scan root, then takes the tail after the last
/// `/crates/` segment; falls back to the raw path if either step can't run.
fn crate_relative(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy().replace('\\', "/");
    match s.rsplit_once("/crates/") {
        Some((_, rel)) => rel.to_string(),
        None => s,
    }
}

/// Is this file a DOCUMENTED exemption, matched by its crate-relative path (not bare
/// filename, so an exemption can't leak across crates)?
fn is_exempt(path: &Path) -> bool {
    EXEMPT.contains(&crate_relative(path).as_str())
}

/// `(file, 1-based line, line text)` for every banned bare-pool write in `files`,
/// excluding the documented [`EXEMPT`] files.
fn offenders(files: &[PathBuf]) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    for file in files {
        if is_exempt(file) {
            continue;
        }
        let body = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", file.display()));
        for (i, line) in body.lines().enumerate() {
            if is_bare_pool_write(line) {
                hits.push((file.display().to_string(), i + 1, line.trim().to_string()));
            }
        }
    }
    hits
}

#[test]
fn no_bare_pool_writes_outside_documented_exceptions() {
    // First, prove the detector actually fires (and doesn't over-fire) — a
    // disk-independent self-check so a broken scan can't pass as a false green: a
    // green exit is not proof, only an exercised assertion is.
    assert!(
        is_bare_pool_write("        .execute(&self.pool)"),
        "the guard's detector failed to flag a planted bare-pool write — it is broken, \
         so a real violation could slip through"
    );
    assert!(
        is_bare_pool_write("        .execute( &self.pool )"),
        "the guard's detector failed to flag a whitespace-spaced bare-pool write \
         `.execute( &self.pool )` — detection must be whitespace-insensitive, or a \
         reformatted violation slips through"
    );
    assert!(
        !is_bare_pool_write("        .execute(&mut *self.conn)"),
        "the guard's detector flagged a legitimate on-transaction write — it would reject \
         the correct Unit-of-Work pattern"
    );

    // Now scan the real write adapters.
    let mut files = Vec::new();
    for dir in scanned_src_dirs() {
        files.extend(rust_files(&dir));
    }
    assert!(
        files.len() > 3,
        "guard scanned suspiciously few files ({}) — check the paths",
        files.len()
    );

    let hits = offenders(&files);
    assert!(
        hits.is_empty(),
        "bare-pool write(s) found — a private-store write must go through the Unit of Work \
         (a transaction-bound view: `uow.accounts().…`), not `{BANNED}`. \
         If the write has no transactional home (an externally-bound `&self` store), add the \
         file to EXEMPT *with a reason*. Offenders:\n{}",
        hits.iter()
            .map(|(f, l, t)| format!("  {f}:{l}  {t}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
