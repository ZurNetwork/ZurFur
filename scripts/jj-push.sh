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
# In order: an argument guard, the migration-name check, the private-papers
# check, `cargo fmt --all --check` and `cargo clippy --workspace --all-targets
# --locked -- -D warnings` (the same commands as CI's fmt and clippy jobs), then
# `jj git push`. Tests stay on CI, exactly as before.
#
# Arguments are NOT forwarded verbatim. The guard admits a whitelist of
# `jj git push` options and refuses everything else (exit 2) before any check
# or push runs: an option that changes which repo, operation or config jj
# uses (`-R`, `--at-op`, `--config`, …) would let the push reach commits the
# papers check never saw. `-c`/`--named` are admitted only for `@`. The push
# always goes to origin, the remote the papers check measures against.
#
# The guard also builds the heads this push names: `@`, plus the bookmark of a
# plain `-b NAME`, all bookmarks for a `-b` pattern, `--all`, `--tracked` or
# jj's default push, all tags for `-t`, and the commits `-r` resolves to. The
# papers check covers what every commit not on origin that those heads reach
# adds or changes. The cargo gate runs against the WORKING COPY: `cargo` reads
# the files on disk, which are the content of `@`.
#
# Invoke directly (`scripts/jj-push.sh ...`) or via the `jj push` alias set in the
# user's jj config. Reversible: delete this file + drop the alias to go back to
# manual gating.

set -eu

# Run from the workspace root, where the Cargo workspace lives. Prefer jj's own
# answer; fall back to git (colocated repo) so the script also works pre-jj.
ROOT=$(jj workspace root 2>/dev/null || git rev-parse --show-toplevel)
cd "$ROOT"

# Argument guard: a whitelist of `jj git push` options; anything else is
# refused before any check or push. It runs after the `cd`, so a revset given
# to `-c`/`--named` resolves here exactly as `jj git push` will resolve it.
refuse() {
    echo "jj-push: refused: $*" >&2
    exit 2
}

at_commit=""
# only_at <option> <revset>: refuse unless the revset is exactly `@`'s commit.
only_at() {
    if [ -z "$at_commit" ]; then
        at_commit=$(jj log -r @ --no-graph -T commit_id) || refuse "could not resolve \`@\`"
    fi
    resolved=$(jj --ignore-working-copy log -r "$2" --no-graph -T 'commit_id ++ "\n"' 2>/dev/null) || resolved=""
    if [ "$resolved" != "$at_commit" ]; then
        refuse "\`$1 $2\`: jj push pushes \`-c\` only for \`@\`; bookmark the change and push the bookmark instead"
    fi
}

# heads is the revset of what this push names; the papers check covers every
# commit not on origin it reaches. `@` is always in it (it covers `-c @` and
# `--named n=@`). default_push stays 1 while no option selects what to push,
# which means jj's default push of tracked bookmarks: all bookmarks.
heads="@"
default_push=1

# value <option> <value>: check one option's value and add what it names to
# heads.
value() {
    case "$2" in
        "" | -*) refuse "\`$1\` needs a value, got \`$2\`" ;;
    esac
    case "$1" in
        -b | --bookmark)
            default_push=0
            # A plain name selects one bookmark; a `kind:` pattern or a glob
            # metacharacter may select many, so the check takes all of them.
            case "$2" in
                *[!A-Za-z0-9._/+-]*) heads="$heads | bookmarks()" ;;
                *) heads="$heads | bookmarks(exact:\"$2\")" ;;
            esac
            ;;
        -r | --revision)
            default_push=0
            # Resolved to commit ids here rather than pasted into the revset,
            # so the value cannot reshape the check's range.
            ids=$(jj log -r "$2" --no-graph -T 'commit_id ++ "\n"') || refuse "\`$1 $2\`: not a revision jj can resolve"
            for id in $ids; do
                heads="$heads | $id"
            done
            ;;
        -t | --tag)
            default_push=0
            heads="$heads | tags()"
            ;;
        --remote)
            [ "$2" = origin ] || refuse "\`--remote $2\`: jj push pushes only to origin, the remote the papers check measures against"
            remote_given=1
            ;;
        -c | --change)
            default_push=0
            only_at "$1" "$2"
            ;;
        --named)
            default_push=0
            case "$2" in
                ?*=?*) only_at "$1" "${2#*=}" ;;
                *) refuse "\`--named $2\`: expected NAME=REVISION" ;;
            esac
            ;;
    esac
}

remote_given=0
pending=""
for arg do
    if [ -n "$pending" ]; then
        value "$pending" "$arg"
        pending=""
        continue
    fi
    case "$arg" in
        -b | --bookmark | -r | --revision | -t | --tag | --color | --remote | -c | --change | --named)
            pending="$arg"
            ;;
        --bookmark=* | --revision=* | --tag=* | --color=* | --remote=* | --change=* | --named=*)
            value "${arg%%=*}" "${arg#*=}"
            ;;
        --all | --tracked)
            default_push=0
            heads="$heads | bookmarks()"
            ;;
        --deleted) default_push=0 ;;
        --dry-run | --allow-empty-description | --allow-private | --allow-conflicts | --quiet | --no-pager | -h | --help) ;;
        -[brtc]?*)
            short=$(printf '%s' "$arg" | cut -c1-2)
            rest=$(printf '%s' "$arg" | cut -c3- | command sed 's/^=//')
            refuse "\`$arg\`: use \`$short $rest\`"
            ;;
        -*) refuse "\`$arg\` is not an option jj push forwards" ;;
        *) refuse "stray argument \`$arg\`" ;;
    esac
done
[ -z "$pending" ] || refuse "\`$pending\` needs a value"
if [ "$default_push" = 1 ]; then
    heads="$heads | bookmarks()"
fi

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
# Only NEW migrations over trunk are checked (the `A` lines of --summary), so
# already-merged offenders don't block — not even when a comment in one is edited.
if new_migrations=$(jj diff --from 'trunk()' --to '@' --summary 2>/dev/null | awk '$1 == "A" { print $2 }' | grep 'crates/adapter-pg/migrations/'); then
    offenders=$(printf '%s\n' "$new_migrations" | grep -E '/[0-9]{8}[0-9]{2}(00|30)00_[^/]*\.sql$' || true)
    if [ -n "$offenders" ]; then
        echo "jj-push: migration timestamp looks hand-typed (round hour/half-hour) — mint it with 'just migrate-add <name>' and move the SQL over:"
        printf '  %s\n' $offenders
        exit 1
    fi
fi

# Private working papers never reach this public repo. The check looks at what
# every commit not on origin that this push names (heads, from the guard)
# adds or changes (`private-papers-check.sh --unpushed`), so it covers
# whatever the push selects, not just `@`; a paper added in one commit and
# deleted in the next is still caught.
echo "jj-push: private-papers-check --unpushed"
sh scripts/private-papers-check.sh --unpushed "$heads" || {
    status=$?
    echo ""
    if [ "$status" = 1 ]; then
        echo "jj-push: commits this push names add or change private working papers — take the papers out of those commits, or push only bookmarks that do not reach them."
    else
        echo "jj-push: the private-papers check could not run — nothing was pushed."
    fi
    exit "$status"
}

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

    echo "jj-push: cargo clippy --locked"
    if ! cargo clippy --workspace --all-targets --locked -- -D warnings; then
        echo ""
        echo "jj-push: clippy failed — fix the warnings above and push again."
        exit 1
    fi
else
    echo "jj-push: no Rust-shaped changes over trunk — skipping fmt/clippy."
fi

echo "jj-push: gate green — pushing."
if [ "$remote_given" = 1 ]; then
    exec jj git push "$@"
fi
exec jj git push --remote origin "$@"
