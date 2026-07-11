//! Prepare-at-codegen: generate **typed query functions** from a crate's
//! `queries/` tree by describing every statement against a live, migrated
//! PostgreSQL — the v2 of the SQL-file convention (Engineer decision
//! 2026-07-11).
//!
//! For each `queries/<namespace>/<name>.sql` the generator asks Postgres
//! (`Executor::describe`, the same machinery sqlx's retired macros used) for the
//! statement's parameter types, result columns, and nullability, merges that
//! with the file's annotation header, and emits into `src/queries.rs`:
//!
//! - one `pub async fn <name>(conn: impl PgExecutor<'_>, …typed params…)` per
//!   statement — bind **arity and types** are now compile-checked at every call
//!   site, and parameters carry the names the annotation declares;
//! - `#[derive(FromRow)]` row structs derived from the *actual* prepared
//!   statement metadata — the row shape has a single source of truth again (the
//!   SQL), closing the DRY gap the hand-written structs carried;
//! - `WRITE_QUERY_FNS` — the namespaced names of every INSERT/UPDATE/DELETE
//!   statement, consumed by the `no_bare_pool_writes` guard so write
//!   classification is structural, not regex.
//!
//! The output is **committed** (`src/queries.rs`), so the workspace still builds
//! with no database; staleness is caught by the `codegen_current` test, which
//! regenerates against a fresh container and diffs — a visible failure, never a
//! silent false green.
//!
//! # Annotation header
//!
//! Leading `--` comment lines of each `.sql` file:
//!
//! ```sql
//! -- params: account_id, invited_user, state    -- $1..$N names; `?` = Option
//! -- fetch: optional                            -- execute | one | optional | many
//! -- row: InvitationRow                         -- shared struct name (optional)
//! -- not_null: count                            -- override unknown nullability
//! -- timestamptz: time                          -- time::OffsetDateTime, not chrono
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, bail, ensure};
use sqlx::TypeInfo;
use sqlx::postgres::PgConnection;
use sqlx::{AssertSqlSafe, Either, Executor, SqlSafeStr};

/// How the statement's result is consumed — from the `-- fetch:` annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fetch {
    /// No result rows: the function returns `rows_affected`.
    Execute,
    /// Exactly one row.
    One,
    /// Zero or one row.
    Optional,
    /// Any number of rows.
    Many,
}

/// One statement's parsed annotation header.
struct Annotations {
    /// `$1..$N` parameter names, `?`-suffixed ones are `Option`.
    params: Vec<(String, bool)>,
    fetch: Fetch,
    /// Explicit row-struct name (shared across statements with equal shapes).
    row: Option<String>,
    /// Columns whose unknown/nullable inference is overridden to NOT NULL.
    not_null: Vec<String>,
    /// Map `timestamptz` to `time::OffsetDateTime` instead of chrono.
    time_crate: bool,
}

/// Parse the leading `--` header of a query file.
fn parse_annotations(path: &Path, sql: &str) -> anyhow::Result<Annotations> {
    let mut params = Vec::new();
    let mut fetch = None;
    let mut row = None;
    let mut not_null = Vec::new();
    let mut time_crate = false;

    for line in sql.lines() {
        let Some(comment) = line.trim().strip_prefix("--") else {
            break; // the header ends at the first non-comment line
        };
        let Some((key, value)) = comment.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "params" => {
                for name in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    match name.strip_suffix('?') {
                        Some(base) => params.push((base.to_string(), true)),
                        None => params.push((name.to_string(), false)),
                    }
                }
            }
            "fetch" => {
                fetch = Some(match value {
                    "execute" => Fetch::Execute,
                    "one" => Fetch::One,
                    "optional" => Fetch::Optional,
                    "many" => Fetch::Many,
                    other => bail!("{}: unknown fetch mode {other:?}", path.display()),
                })
            }
            "row" => row = Some(value.to_string()),
            "not_null" => {
                not_null.extend(value.split(',').map(str::trim).map(String::from));
            }
            "timestamptz" => time_crate = value == "time",
            other => bail!("{}: unknown annotation key {other:?}", path.display()),
        }
    }

    Ok(Annotations {
        params,
        fetch: fetch.with_context(|| format!("{}: missing -- fetch:", path.display()))?,
        row,
        not_null,
        time_crate,
    })
}

/// Map a Postgres type name (from `describe`) to the Rust types the generated
/// code uses: `(parameter_type, column_type)`.
fn rust_types(pg: &str, time_crate: bool) -> anyhow::Result<(&'static str, &'static str)> {
    Ok(match pg {
        "UUID" => ("uuid::Uuid", "uuid::Uuid"),
        "TEXT" | "VARCHAR" => ("&str", "String"),
        "TIMESTAMPTZ" if time_crate => ("time::OffsetDateTime", "time::OffsetDateTime"),
        "TIMESTAMPTZ" => (
            "chrono::DateTime<chrono::Utc>",
            "chrono::DateTime<chrono::Utc>",
        ),
        "INT8" => ("i64", "i64"),
        "INT4" => ("i32", "i32"),
        "BOOL" => ("bool", "bool"),
        "BYTEA" => ("&[u8]", "Vec<u8>"),
        "JSONB" => ("&serde_json::Value", "serde_json::Value"),
        "TEXT[]" => ("&[String]", "Vec<String>"),
        other => bail!("no Rust mapping for Postgres type {other:?}"),
    })
}

/// A generated row struct: its field list as `(name, rust_type)`.
type RowShape = Vec<(String, String)>;

/// Generate the full `src/queries.rs` module text for `queries_dir`, describing
/// every statement against the (migrated) database behind `conn`.
pub async fn generate(conn: &mut PgConnection, queries_dir: &Path) -> anyhow::Result<String> {
    let mut namespaces: Vec<_> = std::fs::read_dir(queries_dir)
        .with_context(|| format!("cannot read {}", queries_dir.display()))?
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    namespaces.sort();

    let mut out = String::from(
        "//! @generated by query-codegen — DO NOT EDIT; run `just gen-queries` to refresh.\n\
         //!\n\
         //! Typed query functions, one per `queries/<namespace>/<name>.sql`, with\n\
         //! parameter and row types read from the live, migrated schema at generation\n\
         //! time (`Executor::describe`). Staleness is caught by the `codegen_current`\n\
         //! test, which regenerates against a fresh container and diffs this file.\n\
         #![allow(clippy::too_many_arguments)]\n",
    );
    let mut write_fns: Vec<String> = Vec::new();

    for ns_dir in namespaces {
        ensure!(
            ns_dir.is_dir(),
            "stray file in queries/: {}",
            ns_dir.display()
        );
        let ns = ns_dir
            .file_name()
            .expect("ns name")
            .to_string_lossy()
            .into_owned();
        let mut files: Vec<_> = std::fs::read_dir(&ns_dir)?
            .map(|entry| entry.expect("dir entry").path())
            .collect();
        files.sort();

        let mut rows: BTreeMap<String, RowShape> = BTreeMap::new();
        let mut fns = String::new();

        for file in &files {
            ensure!(
                file.extension().is_some_and(|e| e == "sql"),
                "non-.sql file: {}",
                file.display()
            );
            let stem = file
                .file_stem()
                .expect("stem")
                .to_string_lossy()
                .into_owned();
            let sql = std::fs::read_to_string(file)?;
            let ann = parse_annotations(file, &sql)?;

            let described = (&mut *conn)
                .describe(AssertSqlSafe(sql.clone()).into_sql_str())
                .await
                .with_context(|| {
                    format!("{} does not describe against the schema", file.display())
                })?;

            // --- parameters ---
            let param_types: Vec<String> = match &described.parameters {
                Some(Either::Left(types)) => types
                    .iter()
                    .map(|t| rust_types(t.name(), ann.time_crate).map(|(p, _)| p.to_string()))
                    .collect::<anyhow::Result<_>>()?,
                Some(Either::Right(n)) => bail!(
                    "{}: driver reported only a parameter count ({n}); types required",
                    file.display()
                ),
                None => Vec::new(),
            };
            ensure!(
                param_types.len() == ann.params.len(),
                "{}: statement has {} parameters but `-- params:` names {}",
                file.display(),
                param_types.len(),
                ann.params.len()
            );
            let params_sig: String = ann
                .params
                .iter()
                .zip(&param_types)
                .map(|((name, optional), ty)| {
                    if *optional {
                        format!(", {name}: Option<{ty}>")
                    } else {
                        format!(", {name}: {ty}")
                    }
                })
                .collect();
            let binds: String = ann
                .params
                .iter()
                .map(|(name, _)| format!(".bind({name})"))
                .collect();

            // --- classification for the bare-pool-write guard ---
            let first_word = sql
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with("--"))
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_uppercase();
            if matches!(first_word.as_str(), "INSERT" | "UPDATE" | "DELETE") {
                write_fns.push(format!("{ns}::{stem}"));
            }

            // --- result shape ---
            let include = format!("include_str!(\"../queries/{ns}/{stem}.sql\")");
            let doc = format!(
                "    /// `queries/{ns}/{stem}.sql`, typed against the migrated schema at generation time."
            );
            if ann.fetch == Fetch::Execute {
                ensure!(
                    described.columns.is_empty(),
                    "{}: fetch: execute but the statement returns columns",
                    file.display()
                );
                let _ = writeln!(
                    fns,
                    "{doc}\n    pub async fn {stem}(conn: impl sqlx::PgExecutor<'_>{params_sig}) -> sqlx::Result<u64> {{\n        sqlx::query({include}){binds}.execute(conn).await.map(|r| r.rows_affected())\n    }}\n"
                );
                continue;
            }

            let mut shape: RowShape = Vec::new();
            for (i, col) in described.columns.iter().enumerate() {
                use sqlx::Column;
                let name = col.name().to_string();
                let (_, base) = rust_types(col.type_info().name(), ann.time_crate)?;
                // Unknown nullability (expressions like count(*)/EXISTS) is treated
                // as nullable unless the annotation overrides it.
                let nullable = described.nullable.get(i).copied().flatten().unwrap_or(true);
                let ty = if nullable && !ann.not_null.iter().any(|n| n == &name) {
                    format!("Option<{base}>")
                } else {
                    base.to_string()
                };
                shape.push((name, ty));
            }
            ensure!(
                !shape.is_empty(),
                "{}: row-returning fetch but no columns",
                file.display()
            );

            let (out_ty, runner, finish) = match ann.fetch {
                Fetch::One => ("{T}".to_string(), "fetch_one", ""),
                Fetch::Optional => ("Option<{T}>".to_string(), "fetch_optional", ""),
                Fetch::Many => ("Vec<{T}>".to_string(), "fetch_all", ""),
                Fetch::Execute => unreachable!(),
            };
            let _ = finish;

            if shape.len() == 1 && ann.row.is_none() {
                // Single column, no named row: a scalar function.
                let scalar_ty = shape[0].1.clone();
                let ret = out_ty.replace("{T}", &scalar_ty);
                let _ = writeln!(
                    fns,
                    "{doc}\n    pub async fn {stem}(conn: impl sqlx::PgExecutor<'_>{params_sig}) -> sqlx::Result<{ret}> {{\n        sqlx::query_scalar({include}){binds}.{runner}(conn).await\n    }}\n"
                );
            } else {
                let row_name = ann.row.clone().unwrap_or_else(|| {
                    let mut pascal = String::new();
                    for part in stem.split('_') {
                        let mut chars = part.chars();
                        if let Some(first) = chars.next() {
                            pascal.push(first.to_ascii_uppercase());
                            pascal.push_str(chars.as_str());
                        }
                    }
                    format!("{pascal}Row")
                });
                if let Some(existing) = rows.get(&row_name) {
                    ensure!(
                        existing == &shape,
                        "{}: row {row_name} redefined with a different shape",
                        file.display()
                    );
                } else {
                    rows.insert(row_name.clone(), shape);
                }
                let ret = out_ty.replace("{T}", &row_name);
                let _ = writeln!(
                    fns,
                    "{doc}\n    pub async fn {stem}(conn: impl sqlx::PgExecutor<'_>{params_sig}) -> sqlx::Result<{ret}> {{\n        sqlx::query_as({include}){binds}.{runner}(conn).await\n    }}\n"
                );
            }
        }

        let _ = writeln!(out, "\npub mod {ns} {{");
        for (name, shape) in &rows {
            let _ = writeln!(
                out,
                "    /// Row shape read back from the prepared statement's metadata.\n    #[derive(Debug, sqlx::FromRow)]\n    pub struct {name} {{"
            );
            for (field, ty) in shape {
                let _ = writeln!(out, "        pub {field}: {ty},");
            }
            out.push_str("    }\n\n");
        }
        out.push_str(&fns);
        out.push_str("}\n");
    }

    // The structural write classification the no_bare_pool_writes guard consumes.
    out.push_str(
        "\n/// Every INSERT/UPDATE/DELETE statement, as `namespace::function` — generated\n\
         /// classification consumed by the `no_bare_pool_writes` guard.\n\
         pub static WRITE_QUERY_FNS: &[&str] = &[\n",
    );
    for name in &write_fns {
        let _ = writeln!(out, "    {name:?},");
    }
    out.push_str("];\n");

    Ok(out)
}
