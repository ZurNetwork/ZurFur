//! CI guard for the compile-enforced Unit of Work (DD `24150017`).
//!
//! The type system already makes the *common* mistake unrepresentable: a write
//! method is only reachable on the transaction-bound [`UnitOfWork`] handle, which
//! holds no pool, so a handler cannot issue a pool-backed write. This test closes
//! the one residual hole the type system can't see — an *adapter author*
//! re-introducing a bare-pool write inside an adapter (where the pool is
//! legitimately in scope for reads).
//!
//! Since prepare-at-codegen, write classification is **structural**, not regex:
//! the generated [`adapter_pg::queries::WRITE_QUERY_FNS`] names every
//! INSERT/UPDATE/DELETE statement (the generator reads the SQL itself), and this
//! guard requires each call to one of those functions, in a non-exempt file, to
//! execute on the transaction-bound `&mut *self.conn` — never on `&self.pool`.
//! A raw `sqlx::query(...).execute(&self.pool)` (bypassing the generated
//! functions entirely) stays banned by the original textual rule.
//!
//! **Reads on the pool are fine** and untouched: only statements the generator
//! classified as writes are inspected.
//!
//! [`UnitOfWork`]: domain::ports::UnitOfWork

use std::path::{Path, PathBuf};

/// The raw bare-pool *write* signature this guard bans (the pre-codegen rule,
/// kept for anything bypassing the generated functions), in its canonical
/// whitespace-free form.
const BANNED_RAW: &str = ".execute(&self.pool)";

/// The only receiver a generated write function may take in a non-exempt file,
/// whitespace-stripped: the write view's borrowed transaction connection.
const TX_RECEIVER: &str = "(&mut*self.conn";

/// Files allowed to keep pool-backed writes, keyed by **crate-relative path** — not
/// bare filename, so an exemption can't leak across crates. Each is a DOCUMENTED
/// exception, not a silent skip: it writes on the pool because it has **no
/// transactional invariant to uphold** — there is no domain unit of work to thread
/// through:
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
/// - `adapter-pg/src/key_store.rs` — the `did:plc` custody-key persistence
///   (`PgKeyStore::put`, ZMVP-49). Both this write and the account row it belongs to
///   are the *same* private Postgres store, so this is **not** a cross-store concern;
///   the exemption is **same-store temporal ordering**. The write happens *inside
///   minting*, **before** the account row exists — the account's DID is *derived
///   from* the very keys being stored — so there is no account transaction yet to
///   join. One row, no cross-aggregate invariant. (Ratified.)
/// - `adapter-pg/src/plc_operation_log.rs` — the `did:plc` operation-log persistence
///   (`PgPlcOperationLog::append`, ZMVP-34). Same rationale as `key_store`, at its two
///   write points, **neither** of which has an account transaction to join: the
///   *genesis* op is logged during minting, before the account row exists (alongside
///   the keys); the *tombstone* op is logged during hard-delete as the private half of
///   the separate, retryable public submission step — by which point the account row is
///   already gone. Same-store, single-row, no cross-aggregate invariant.
/// - `adapter-pg/src/file_store.rs` — the commission file-entry **blob** store
///   (`PgFileStore::put`/`delete`, ZMVP-88, ruling E13). The blob write has **no
///   transactional home**: bytes cannot ride a Postgres unit of work, and the file
///   entry's atomicity lives in the *other* two writes — the `commission_file` link
///   and the `file_added` changelog entry — which commit together in the UnitOfWork.
///   The blob `put` runs **before** that unit as its own step (orphan-on-rollback is
///   accepted and recorded — nothing points at an orphan), the same "public write is
///   its own retryable step" posture the PDS mirror uses. Single-statement, no
///   cross-aggregate invariant. (Ruling E13.)
///
/// If a *new* file needs to be exempted, that is a design question (does its write
/// truly have no transactional home?), not a quiet edit to this list.
const EXEMPT: &[&str] = &[
    "adapter-pg/src/session_store.rs",
    "adapter-atproto/src/auth_store.rs",
    "adapter-pg/src/profile.rs",
    "adapter-pg/src/key_store.rs",
    "adapter-pg/src/plc_operation_log.rs",
    "adapter-pg/src/file_store.rs",
];

/// The private-store write adapters this guard scans, relative to this crate's
/// `CARGO_MANIFEST_DIR` (`backend/crates/adapter-pg`). `adapter-atproto` is included
/// because its `auth_store` persists OAuth state to Postgres on a pool, so it is a
/// pg-writing adapter even though it lives behind the public boundary. Generated
/// modules (`src/queries.rs`) are skipped: they define the functions; call sites are
/// what the guard judges.
fn scanned_src_dirs() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    vec![
        manifest.join("src"),
        manifest.join("../adapter-atproto/src"),
    ]
}

/// Every `*.rs` file under `dir`, recursively, minus the generated query modules.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_none_or(|n| n != "queries.rs")
        {
            out.push(path);
        }
    }
    out
}

/// The bare function names of every generated write statement (the part after
/// `namespace::`), across BOTH scanned crates — adapter-atproto's writes are
/// classified too, so a future non-exempt module there can't slip a pool-backed
/// write past the guard (PR #119 review).
fn write_fn_names() -> Vec<&'static str> {
    adapter_pg::queries::WRITE_QUERY_FNS
        .iter()
        .chain(adapter_atproto::queries::WRITE_QUERY_FNS)
        .map(|path| path.rsplit_once("::").map_or(*path, |(_, name)| name))
        .collect()
}

/// Whitespace-stripped form of a source chunk — the canonical matching space.
fn collapsed(chunk: &str) -> String {
    chunk.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

/// Does this `;`-terminated statement chunk violate the Unit-of-Work rule?
/// Either the raw pre-codegen ban (`.execute(&self.pool)`), or a call to a
/// generated **write** function whose executor is not the transaction-bound
/// `&mut *self.conn`.
fn is_bare_pool_write(chunk: &str, write_fns: &[&str]) -> bool {
    let collapsed = collapsed(chunk);
    if collapsed.contains(BANNED_RAW) {
        return true;
    }
    write_fns
        .iter()
        .any(|name| collapsed.contains(&format!("::{name}(")))
        && !collapsed.contains(TX_RECEIVER)
        && collapsed.contains("self.pool")
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

/// `(file, chunk)` for every banned bare-pool write in `files`, excluding the
/// documented [`EXEMPT`] files.
fn offenders(files: &[PathBuf], write_fns: &[&str]) -> Vec<(String, String)> {
    let mut hits = Vec::new();
    for file in files {
        if is_exempt(file) {
            continue;
        }
        let body = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", file.display()));
        for chunk in body.split(';') {
            if is_bare_pool_write(chunk, write_fns) {
                hits.push((
                    file.display().to_string(),
                    chunk.trim().chars().take(200).collect(),
                ));
            }
        }
    }
    hits
}

#[test]
fn no_bare_pool_writes_outside_documented_exceptions() {
    let write_fns = write_fn_names();
    assert!(
        write_fns.len() > 20,
        "suspiciously few generated write functions ({}) — the WRITE_QUERY_FNS \
         classification looks hollowed out",
        write_fns.len()
    );

    // First, prove the detector actually fires (and doesn't over-fire) — a
    // disk-independent self-check so a broken scan can't pass as a false green: a
    // green exit is not proof, only an exercised assertion is.
    assert!(
        is_bare_pool_write("        .execute(&self.pool)", &write_fns),
        "the guard's detector failed to flag a planted raw bare-pool write"
    );
    assert!(
        is_bare_pool_write(
            "sql::create_account(&self.pool, id, did, handle, name, a, b).await",
            &write_fns
        ),
        "the guard's detector failed to flag a generated WRITE function executed on \
         the pool — the structural rule is broken"
    );
    assert!(
        !is_bare_pool_write(
            "sql::create_account(&mut *self.conn, id, did, handle, name, a, b).await",
            &write_fns
        ),
        "the guard's detector flagged a legitimate on-transaction write — it would \
         reject the correct Unit-of-Work pattern"
    );
    assert!(
        !is_bare_pool_write("sql::find(&self.pool, id).await", &write_fns),
        "the guard's detector flagged a pool-backed READ — reads on the pool are fine"
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

    let hits = offenders(&files, &write_fns);
    assert!(
        hits.is_empty(),
        "bare-pool write(s) found — a private-store write must go through the Unit of \
         Work (a transaction-bound view executing on `&mut *self.conn`), never on the \
         pool. If the write has no transactional home (an externally-bound `&self` \
         store), add the file to EXEMPT *with a reason*. Offenders:\n{}",
        hits.iter()
            .map(|(f, t)| format!("  {f}\n    {t}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
