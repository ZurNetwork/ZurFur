#!/usr/bin/env bash
# The light local gate for restacks and fix rounds: fmt, clippy, typos and the
# design-index check always; the tests of the crates the change touches; the
# web checks when the change touches the frontend or the contract. CI re-runs
# the full suite on every push. Run via `just gate-light`.
#
# The change is the diff from trunk() to @ (jj), or from the merge base with
# origin/main (git). Fails closed: if the diff can't be read, or a path is
# neither docs-only (scripts/ci-paths.sh) nor inside one crate, the whole
# workspace is tested and the web checks run. `--plan` prints the steps
# without running them.
set -euo pipefail

plan=0
case "${1:-}" in
    "") ;;
    --plan) plan=1 ;;
    *)
        echo "usage: gate-light.sh [--plan]" >&2
        exit 2
        ;;
esac

root="$(jj workspace root 2>/dev/null || git rev-parse --show-toplevel)"
cd "$root"

if ! changed="$(jj diff --from 'trunk()' --to '@' --name-only 2>/dev/null)"; then
    if ! base="$(git merge-base origin/main HEAD 2>/dev/null)" ||
        ! changed="$(git diff --no-renames --name-only "$base" 2>/dev/null)"; then
        changed=""
        echo "gate-light: cannot read the change; running everything"
        unknown=1
    fi
fi

whole_workspace="${unknown:-0}"
web="${unknown:-0}"
declare -A crates=()

# Classify first, so a failing classifier runs everything rather than nothing.
if [ "$whole_workspace" = 0 ] &&
    ! classified="$(printf '%s\n' "$changed" | sh scripts/ci-paths.sh --paths)"; then
    echo "gate-light: the path classifier failed; running everything"
    whole_workspace=1
    web=1
fi

# crate_package <dir> prints the package name in <dir>/Cargo.toml.
crate_package() {
    awk -F'"' '/^name[[:space:]]*=/ { print $2; exit }' "$1/Cargo.toml"
}

if [ "$whole_workspace" = 0 ]; then
    while read -r kind path; do
        [ "$kind" = code ] || continue
        case "$path" in
            frontend/*)
                web=1
                ;;
            contract/*)
                whole_workspace=1
                web=1
                ;;
            backend/crates/*/*)
                dir="${path#backend/crates/}"
                dir="backend/crates/${dir%%/*}"
                if [ -f "$dir/Cargo.toml" ]; then
                    crates["$(crate_package "$dir")"]=1
                else
                    whole_workspace=1
                fi
                ;;
            *)
                whole_workspace=1
                ;;
        esac
    done <<<"$classified"
fi

step() {
    echo "gate-light: $*"
    [ "$plan" = 1 ] || "$@"
}

step cargo fmt --all --check
step cargo clippy --workspace --all-targets --locked -- -D warnings
step typos
step just design-index-check

if [ "$whole_workspace" = 1 ]; then
    INSTA_UPDATE=no step cargo test --workspace --locked
elif [ "${#crates[@]}" -gt 0 ]; then
    packages=()
    for name in "${!crates[@]}"; do
        packages+=(-p "$name")
    done
    INSTA_UPDATE=no step cargo test --locked "${packages[@]}"
else
    echo "gate-light: no crate changed; skipping cargo test"
fi

if [ "$web" = 1 ]; then
    step yarn --cwd frontend/web run check
    step yarn --cwd frontend/web run lint
    step yarn --cwd frontend/web run test
    step yarn --cwd frontend/web run build
else
    echo "gate-light: frontend/ unchanged; skipping the web checks"
fi

if [ "$plan" = 1 ]; then
    echo "gate-light: plan only; nothing ran"
else
    echo "gate-light: green (CI runs the full suite)"
fi
