#!/bin/sh
# Tests for scripts/build-id.sh: jj first, then git, then "unknown"; the
# "(local changes)" suffix; exactly one line on stdout and nothing on stderr.
# Stub `jj` and `git` stand in for the real tools on a PATH holding nothing
# else. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/build-id.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
failures=0

# ok <name> / fail <name> <why>
ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# A `jj` that prints $STUB_JJ_OUT, or fails when $STUB_JJ_FAIL is set; it
# complains on stderr either way, which build-id.sh must swallow.
write_jj() {
    cat >"$1/jj" <<'STUB'
#!/bin/sh
echo "jj stub noise" >&2
[ -z "${STUB_JJ_FAIL:-}" ] || exit 1
printf '%s\n' "$STUB_JJ_OUT"
STUB
    chmod +x "$1/jj"
}

# A `git` that answers rev-parse with $STUB_GIT_HEAD (failing when it is
# empty) and status with $STUB_GIT_STATUS.
write_git() {
    cat >"$1/git" <<'STUB'
#!/bin/sh
echo "git stub noise" >&2
case "$1" in
    rev-parse) [ -n "${STUB_GIT_HEAD:-}" ] || exit 128; printf '%s\n' "$STUB_GIT_HEAD" ;;
    status) printf '%s' "${STUB_GIT_STATUS:-}" ;;
    *) exit 1 ;;
esac
STUB
    chmod +x "$1/git"
}

both="$work/both"
git_only="$work/git-only"
neither="$work/neither"
mkdir -p "$both" "$git_only" "$neither"
write_jj "$both"
write_git "$both"
write_git "$git_only"

# check <name> <bin dir> <expected line>: build-id.sh with only <bin dir> on
# PATH prints exactly <expected line> on stdout and nothing on stderr.
check() {
    name="$1" bin="$2" want="$3"
    if ! PATH="$bin" /bin/sh "$under_test" >"$work/stdout" 2>"$work/stderr"; then
        fail "$name" "exited non-zero"
        return
    fi
    lines="$(wc -l <"$work/stdout" | tr -d ' ')"
    got="$(cat "$work/stdout")"
    if [ "$lines" != 1 ]; then
        fail "$name" "printed $lines lines: '$got'"
    elif [ "$got" != "$want" ]; then
        fail "$name" "printed '$got', want '$want'"
    elif [ -s "$work/stderr" ]; then
        fail "$name" "wrote to stderr: $(cat "$work/stderr")"
    else
        ok "$name"
    fi
}

export STUB_GIT_HEAD=bbbbbbbbbbbb STUB_GIT_STATUS=""

STUB_JJ_OUT="aaaaaaaaaaaa" check "jj comes first" "$both" "aaaaaaaaaaaa"
STUB_JJ_OUT="aaaaaaaaaaaa (local changes)" check "jj's local-changes suffix passes through" "$both" "aaaaaaaaaaaa (local changes)"
STUB_JJ_FAIL=1 check "git answers when jj fails" "$both" "bbbbbbbbbbbb"
check "git answers when jj is missing" "$git_only" "bbbbbbbbbbbb"
STUB_GIT_STATUS=" M Justfile" check "git adds the local-changes suffix" "$git_only" "bbbbbbbbbbbb (local changes)"
STUB_JJ_OUT="$(printf 'aaaaaaaaaaaa\nsecond line')" check "a two-line jj answer falls through to git" "$both" "bbbbbbbbbbbb"
STUB_JJ_OUT="$(printf 'a%.0s' $(seq 1 65))" check "an id over 64 bytes falls through to git" "$both" "bbbbbbbbbbbb"
STUB_GIT_HEAD="" check "unknown when git has no HEAD" "$git_only" "unknown"
check "unknown when neither tool exists" "$neither" "unknown"

if [ "$failures" -gt 0 ]; then
    echo "$failures build-id check(s) failed"
    exit 1
fi
echo "all build-id checks hold"
