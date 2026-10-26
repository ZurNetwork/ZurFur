#!/bin/sh
# Tests for scripts/cargo-config-check.sh. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/cargo-config-check.sh"
failures=0

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The fixtures are plain git repos, so the script must take its git branch: a
# stub `jj` that always fails stands first on PATH, and git reads no global or
# system config.
mkdir "$tmp/bin"
printf '#!/bin/sh\nexit 1\n' >"$tmp/bin/jj"
chmod +x "$tmp/bin/jj"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# repo <name> [paths to track]...: a fresh git repo at $tmp/<name> holding each
# path, added to the index; prints its path.
repo() {
    dir="$tmp/$1"
    shift
    git init -q "$dir"
    for path in "$@"; do
        mkdir -p "$dir/$(dirname "$path")"
        echo x >"$dir/$path"
        git -C "$dir" add -- "$path"
    done
    printf '%s\n' "$dir"
}

# expect <name> <repo> <expected exit> [text the output must hold]...
expect() {
    name="$1"
    dir="$2"
    want="$3"
    shift 3
    out="$(cd "$dir" && GIT_CEILING_DIRECTORIES="$tmp" PATH="$tmp/bin:$PATH" sh "$under_test" 2>&1)" && got=0 || got=$?
    if [ "$got" != "$want" ]; then
        echo "FAIL $name: expected exit $want, got $got: $out"
        failures=$((failures + 1))
        return
    fi
    for text in "$@"; do
        case "$out" in
            *"$text"*) ;;
            *)
                echo "FAIL $name: output lacks \"$text\": $out"
                failures=$((failures + 1))
                return
                ;;
        esac
    done
    echo "ok   $name"
}

r="$(repo root-config README.md .cargo/config)"
expect "a tracked .cargo/config fails" "$r" 1 "  .cargo/config
"
r="$(repo root-config-toml README.md .cargo/config.toml)"
expect "a tracked .cargo/config.toml fails" "$r" 1 "  .cargo/config.toml
"
r="$(repo deep-config-toml README.md backend/.cargo/config.toml)"
expect "a tracked backend/.cargo/config.toml fails" "$r" 1 "  backend/.cargo/config.toml
"
r="$(repo capitals README.md .Cargo/Config.toml)"
expect "a tracked .Cargo/Config.toml in capitals fails" "$r" 1 "  .Cargo/Config.toml
"
r="$(repo several .cargo/config backend/.cargo/config.toml README.md)"
expect "every tracked cargo config is listed" "$r" 1 "  .cargo/config
" "  backend/.cargo/config.toml
"
r="$(repo quoted 'a"b/.cargo/config.toml')"
expect "a path git quotes is still caught" "$r" 1 '.cargo/config.toml"'

r="$(repo lookalikes README.md .cargo/other cargo/config.toml .cargo/config.toml.bak .cargo/configx x.cargo/config.toml)"
out="$(cd "$r" && GIT_CEILING_DIRECTORIES="$tmp" PATH="$tmp/bin:$PATH" sh "$under_test" 2>&1)" && got=0 || got=$?
if [ "$got" = 0 ] && [ "$out" = "cargo-config-check: OK, no cargo config file tracked" ]; then
    echo "ok   lookalikes (.cargo/other, cargo/config.toml, …) pass with one OK line"
else
    echo "FAIL lookalikes should pass with one OK line: exit $got: $out"
    failures=$((failures + 1))
fi

r="$(repo untracked README.md)"
mkdir "$r/.cargo"
echo x >"$r/.cargo/config.toml"
expect "an untracked .cargo/config.toml passes" "$r" 0 "OK, no cargo config file tracked"

r="$(repo empty)"
expect "an empty listing fails closed" "$r" 2 "the tracked-file listing is empty"

mkdir "$tmp/nowhere"
expect "no jj workspace and no git checkout fails closed" "$tmp/nowhere" 2

if sh "$under_test" --bogus </dev/null >/dev/null 2>&1; then
    echo "FAIL an argument should exit non-zero"
    failures=$((failures + 1))
else
    echo "ok   an argument exits non-zero"
fi

# Today's tree, listed the way the script finds it (jj where present, else git).
out="$(cd "$script_dir/.." && sh "$under_test" 2>&1)" && got=0 || got=$?
if [ "$got" = 0 ]; then
    echo "ok   today's tree passes"
else
    echo "FAIL today's tree: expected exit 0, got $got: $out"
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all cargo-config-check tests passed"
