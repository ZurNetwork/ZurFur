#!/bin/sh
# Fails when a cargo config file is tracked: a `.cargo/config` or
# `.cargo/config.toml` at any depth, in any letter case (a case-insensitive
# file system reads `.Cargo/Config.toml` too). Cargo reads such a file from
# the directory it runs in and every parent, and the file can name a
# `rustc-wrapper`, a linker or a runner for the binaries cargo runs, so a
# change that adds one runs its own code inside every job that calls cargo.
#
# Lists the tracked files with `jj file list` in a jj workspace (the working
# copy, @), else `git ls-files`. Exits 1 and lists each offending path; exits
# 0 with one OK line when clean. Fails closed: exits 2 when it cannot list the
# tracked files or the listing comes back empty.
set -eu

[ "$#" = 0 ] || {
    echo "usage: cargo-config-check.sh" >&2
    exit 2
}

if root=$(jj workspace root 2>/dev/null); then
    cd "$root"
    files=$(jj file list) || {
        echo "cargo-config-check: \`jj file list\` failed (a stale workspace? try \`jj workspace update-stale\`)" >&2
        exit 2
    }
elif root=$(git rev-parse --show-toplevel 2>/dev/null); then
    cd "$root"
    files=$(git -c core.quotePath=false ls-files) || {
        echo "cargo-config-check: \`git ls-files\` failed" >&2
        exit 2
    }
else
    echo "cargo-config-check: not inside a jj workspace or a git checkout" >&2
    exit 2
fi
if [ -z "$files" ]; then
    echo "cargo-config-check: the tracked-file listing is empty" >&2
    exit 2
fi

# A path git had to quote ("…") is unquoted first; the match is on the
# lower-cased path, and a hit prints the path as given.
hits=$(printf '%s\n' "$files" | awk '
    {
        path = $0
        if (path ~ /^".*"$/) path = substr(path, 2, length(path) - 2)
        if (tolower(path) ~ /(^|\/)\.cargo\/config(\.toml)?$/) print "  " $0
    }
') || {
    echo "cargo-config-check: the path check itself failed" >&2
    exit 2
}

if [ -n "$hits" ]; then
    echo "cargo-config-check: these tracked paths are cargo config files, which can run their own code in any cargo call:"
    printf '%s\n' "$hits"
    echo "cargo-config-check: keep cargo config files out of the repo; a local one stays untracked."
    exit 1
fi
echo "cargo-config-check: OK, no cargo config file tracked"
