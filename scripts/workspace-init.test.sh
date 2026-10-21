#!/bin/sh
# Tests for scripts/workspace-init.sh: the isolated `.env` block in a secondary
# jj workspace, its re-run idempotence, the refusal in the primary checkout,
# and the plain-git fallback. Run directly, or via `just scripts-test`. The jj
# cases need jj; without it (CI's `hooks` job) they print a skip line.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/workspace-init.sh"
failures=0
ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf '[user]\nname = "fixture"\nemail = "fixture@example.invalid"\n' >"$tmp/config.toml"
export JJ_CONFIG="$tmp/config.toml" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

write_env_example() {
    cat >"$1/.env.example" <<'EOF'
ZURFUR_PDS_JWT_SECRET=example-jwt
ZURFUR_PDS_ADMIN_PASSWORD=example-admin
ZURFUR_PDS_PLC_ROTATION_KEY=example-rotation
EOF
}

# run_in <dir> — runs the script with <dir> as the working directory; the exit
# code lands in $rc, stdout/stderr in $tmp/out and $tmp/err.
run_in() {
    set +e
    (cd "$1" && bash "$under_test") >"$tmp/out" 2>"$tmp/err"
    rc=$?
    set -e
}

count() { grep -c -- "$1" "$2" || true; }

if command -v jj >/dev/null 2>&1; then
    primary="$tmp/primary"
    secondary="$tmp/ws-one"
    jj git init "$primary" >/dev/null 2>&1
    printf 'ZURFUR_PDS_JWT_SECRET=primary-jwt\nPRIMARY_ONLY=1\n' >"$primary/.env"
    (cd "$primary" && jj workspace add "$secondary" >/dev/null 2>&1)
    write_env_example "$secondary"

    # --- first run in a secondary workspace: seeded .env + one managed block ---
    run_in "$secondary"
    [ "$rc" -eq 0 ] && ok "jj: secondary workspace exits 0" || fail "jj: exit $rc ($(cat "$tmp/err"))"
    env_file="$secondary/.env"
    grep -qx 'PRIMARY_ONLY=1' "$env_file" && ok "jj: .env seeded from the default workspace" || fail "jj: .env not seeded from the primary"
    grep -qx 'ZURFUR_PDS_JWT_SECRET=primary-jwt' "$env_file" && ok "jj: primary secret kept" || fail "jj: primary secret missing"
    grep -qx 'ZURFUR_PDS_ADMIN_PASSWORD=example-admin' "$env_file" && ok "jj: missing secret seeded from .env.example" || fail "jj: missing secret not seeded"
    grep -qx 'COMPOSE_PROJECT_NAME=ws-one' "$env_file" && ok "jj: compose project named after the workspace" || fail "jj: compose project name"
    grep -qF '# >>> workspace isolation (ws-one)' "$env_file" && ok "jj: managed block marker" || fail "jj: managed block marker missing"
    first_block="$(awk '/^# >>> workspace isolation/,/^# <<< workspace isolation/' "$env_file")"

    # --- re-run: still one block, same ports; an older worktree block is replaced ---
    printf '\n# >>> worktree isolation (old)\nOLD_KEY=1\nZURFUR_DB_HOST_PORT=1\n# <<< worktree isolation\n' >>"$env_file"
    run_in "$secondary"
    [ "$rc" -eq 0 ] && ok "jj: re-run exits 0" || fail "jj: re-run exit $rc ($(cat "$tmp/err"))"
    [ "$(count '^# >>> ' "$env_file")" -eq 1 ] && ok "jj: re-run leaves exactly one managed block" || fail "jj: $(count '^# >>> ' "$env_file") managed blocks"
    [ "$(count '^ZURFUR_DB_HOST_PORT=' "$env_file")" -eq 1 ] && ok "jj: each managed key defined once" || fail "jj: ZURFUR_DB_HOST_PORT defined more than once"
    if grep -q 'OLD_KEY\|worktree isolation' "$env_file"; then fail "jj: the older worktree block survived"; else ok "jj: the older worktree block is removed"; fi
    second_block="$(awk '/^# >>> workspace isolation/,/^# <<< workspace isolation/' "$env_file")"
    [ "$first_block" = "$second_block" ] && ok "jj: re-run keeps the same ports" || fail "jj: re-run changed the block"
    grep -qx 'PRIMARY_ONLY=1' "$env_file" && ok "jj: re-run keeps the rest of .env" || fail "jj: re-run dropped unmanaged lines"

    # --- the default workspace is refused and left untouched ---
    before="$(cat "$primary/.env")"
    run_in "$primary"
    [ "$rc" -eq 1 ] && ok "jj: primary checkout exits 1" || fail "jj: primary exit $rc"
    grep -qF 'refusing to isolate the PRIMARY checkout' "$tmp/err" && ok "jj: primary refusal message" || fail "jj: primary refusal message missing"
    [ "$(cat "$primary/.env")" = "$before" ] && ok "jj: primary .env untouched" || fail "jj: primary .env changed"
else
    echo "skip (no jj): the jj workspace fixtures"
fi

# --- plain git with `git worktree add`: the fallback ---
gprimary="$tmp/gprimary"
gsecondary="$tmp/gwt"
git init -q "$gprimary"
git -C "$gprimary" -c user.name=fixture -c user.email=fixture@example.invalid commit -q --allow-empty -m init
git -C "$gprimary" worktree add -q "$gsecondary"
printf 'PRIMARY_ONLY=1\n' >"$gprimary/.env"
write_env_example "$gsecondary"
run_in "$gsecondary"
[ "$rc" -eq 0 ] && ok "git: secondary worktree exits 0" || fail "git: exit $rc ($(cat "$tmp/err"))"
grep -qx 'COMPOSE_PROJECT_NAME=gwt' "$gsecondary/.env" && ok "git: compose project named after the worktree" || fail "git: compose project name"
grep -qx 'PRIMARY_ONLY=1' "$gsecondary/.env" && ok "git: .env seeded from the primary" || fail "git: .env not seeded"
run_in "$gprimary"
[ "$rc" -eq 1 ] && ok "git: primary checkout exits 1" || fail "git: primary exit $rc"

# --- neither jj nor git ---
mkdir "$tmp/plain"
run_in "$tmp/plain"
[ "$rc" -eq 1 ] && ok "plain directory exits 1" || fail "plain directory exit $rc"
grep -qF 'not inside a jj workspace or a git checkout' "$tmp/err" && ok "plain directory message" || fail "plain directory message missing"

if [ "$failures" -eq 0 ]; then
    echo "workspace-init.test.sh: all cases passed"
else
    echo "workspace-init.test.sh: $failures case(s) failed"
    exit 1
fi
