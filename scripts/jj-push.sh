#!/bin/sh
# Pre-push gate for Jujutsu (jj) — the jj-native successor to the git
# `.githooks/pre-commit` hook, honoring the Engineer directive of 2026-07-06:
# formatting + linter run before code leaves the machine; the test suite runs on
# CI. jj deliberately has no native pre-commit hook (jj-vcs/jj#405, open 4+ years:
# pre-commit is git-index-based and jj has no "commit moment"), and the community
# converged on gating at PUSH instead — which also fits jj's grain better, since
# jj makes many tiny working-copy snapshots with no commit ceremony. Gating once,
# at push (the moment code reaches the remote and CI), is the right seam.
#
# Runs the SAME two checks as the old hook — `cargo fmt --all --check` and
# `cargo clippy --workspace --all-targets -- -D warnings` (SQLX_OFFLINE=true) —
# then hands off to `jj git push`. Tests stay on CI, exactly as before.
#
# The gate runs against the WORKING COPY: `cargo` reads the files on disk, which
# are the content of `@`, which is what CI builds for the pushed commit. All
# arguments are forwarded verbatim to `jj git push`, so `jj-push.sh -b <bookmark>`,
# `--all`, `--dry-run`, etc. all work.
#
# Invoke directly (`scripts/jj-push.sh ...`) or via the `jj push` alias set in the
# user's jj config. Reversible: delete this file + drop the alias to go back to
# manual gating.

set -eu

# Run from the workspace root, where the Cargo workspace lives. Prefer jj's own
# answer; fall back to git (colocated repo) so the script also works pre-jj.
ROOT=$(jj workspace root 2>/dev/null || git rev-parse --show-toplevel)
cd "$ROOT"

# Fast path: only spend time on cargo when this branch's diff over trunk actually
# touches Rust-shaped files (mirrors the old hook's staged-file check). Fail-safe:
# if the range can't be computed for any reason, run the gate rather than skip it.
run_gate=1
if changed=$(jj diff --from 'trunk()' --to '@' --name-only 2>/dev/null); then
    if printf '%s\n' "$changed" | grep -qE '\.(rs|toml)$'; then
        run_gate=1
    else
        run_gate=0
    fi
fi

# Deterministic convention check, independent of the cargo gate (a migration-only
# diff has no .rs changes but must still be checked): migrations must be minted by
# `just migrate-add` — sqlx stamps a to-the-second UTC version. A round hour or
# half-hour HHMMSS means the filename was hand-typed, which risks a version-key
# collision with another branch's migration at rebase/integration (see CLAUDE.md).
# Only NEW migrations over trunk are checked, so already-merged offenders don't block.
if new_migrations=$(jj diff --from 'trunk()' --to '@' --name-only 2>/dev/null | grep 'crates/adapter-pg/migrations/'); then
    offenders=$(printf '%s\n' "$new_migrations" | grep -E '/[0-9]{8}[0-9]{2}(00|30)00_[^/]*\.sql$' || true)
    if [ -n "$offenders" ]; then
        echo "jj-push: migration timestamp looks hand-typed (round hour/half-hour) — mint it with 'just migrate-add <name>' and move the SQL over:"
        printf '  %s\n' $offenders
        exit 1
    fi
fi

if [ "$run_gate" = 1 ]; then
    command -v cargo >/dev/null 2>&1 || { echo "jj-push: cargo not found (install the Rust toolchain)"; exit 1; }
    cargo fmt --version >/dev/null 2>&1 || { echo "jj-push: rustfmt not available (run: rustup component add rustfmt)"; exit 1; }
    cargo clippy --version >/dev/null 2>&1 || { echo "jj-push: clippy not available (run: rustup component add clippy)"; exit 1; }

    echo "jj-push: cargo fmt --all --check"
    if ! cargo fmt --all --check; then
        echo ""
        echo "jj-push: formatting failed — run 'cargo fmt --all' and push again."
        exit 1
    fi

    echo "jj-push: cargo clippy (SQLX_OFFLINE=true)"
    if ! SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings; then
        echo ""
        echo "jj-push: clippy failed — fix the warnings above and push again."
        exit 1
    fi
else
    echo "jj-push: no Rust-shaped changes over trunk — skipping fmt/clippy."
fi

echo "jj-push: gate green — pushing."
exec jj git push "$@"
