#!/bin/sh
# Drift check for docs/design-index.md (the generated pointer index) against
# the design corpus (ZurNetwork/zurfur-design, private, checked out at
# $ZURFUR_DESIGN_DIR). Ruled semantics: migration/DECISIONS.md "Drift check"
# row, plan P6 (~/.claude/plans/alright-let-s-plan-this-starry-nova.md).
#
# POSIX sh + awk only — no `sed` (aliased to `sd` on some dev machines).
#
#   corpus absent, $CI set        -> GitHub notice, exit 0 (skipped)
#   corpus absent, $CI unset      -> error, exit 1
#   corpus absent, ZURFUR_DESIGN_SKIP=1, $CI unset -> notice, exit 0
#   docs/design-index.md missing (corpus present)  -> warning, exit 0
#   index behind the corpus (additions/retitles/status changes only) -> warning, exit 0
#   index contradicts the corpus (unknown id, non-generated header, a
#     digit on a non-entry line)  -> error, exit 1
#   --strict: any difference at all (including a mere "behind" warning) -> exit 1
#
# Testing seam: DESIGN_INDEX_GENERATE_CMD, if set, replaces the `cargo run`
# invocation that regenerates the index (used by design-index-check.test.sh
# to avoid compiling the real design crate for pure drift-classification
# cases). Production callers never need to set it.
set -eu

strict=0
for arg in "$@"; do
    case "$arg" in
        --strict) strict=1 ;;
        *)
            echo "design-index-check: unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

: "${ZURFUR_DESIGN_DIR:=$HOME/code/zurfur-design}"
committed="docs/design-index.md"

corpus_present() {
    [ -f "$ZURFUR_DESIGN_DIR/Cargo.toml" ]
}

if ! corpus_present; then
    if [ -n "${CI:-}" ]; then
        echo "::notice::design corpus not present — drift check skipped"
        exit 0
    fi
    if [ "${ZURFUR_DESIGN_SKIP:-}" = "1" ]; then
        echo "notice: design corpus not present at $ZURFUR_DESIGN_DIR — drift check skipped (ZURFUR_DESIGN_SKIP=1)"
        exit 0
    fi
    echo "error: design corpus not present at $ZURFUR_DESIGN_DIR (set ZURFUR_DESIGN_DIR, or ZURFUR_DESIGN_SKIP=1 to skip this check locally)" >&2
    exit 1
fi

generated="$(mktemp)"
contradictions="$(mktemp)"
old_ids="$(mktemp)"
new_ids="$(mktemp)"
cleanup() { rm -f "$generated" "$contradictions" "$old_ids" "$new_ids"; }
trap cleanup EXIT

if [ -n "${DESIGN_INDEX_GENERATE_CMD:-}" ]; then
    # shellcheck disable=SC2086
    eval "$DESIGN_INDEX_GENERATE_CMD" >"$generated"
else
    cargo run -q --manifest-path "$ZURFUR_DESIGN_DIR/Cargo.toml" -p design -- \
        index --root "$ZURFUR_DESIGN_DIR" >"$generated"
fi

if [ ! -f "$committed" ]; then
    echo "warning: $committed does not exist yet — run 'just design-index' once the design corpus has pages to index"
    [ "$strict" -eq 1 ] && exit 1
    exit 0
fi

# Structural check of the committed file's own shape: every line is either a
# digit-free "## <dir>/" header, a "- <id> <status> — <title>" entry, blank,
# or (anything else) must carry no digit at all.
awk '
    /^## / {
        if ($0 ~ /[0-9]/) print "contradiction: non-generated header: " $0
        next
    }
    /^- [0-9]+ / { next }
    /^[[:space:]]*$/ { next }
    { if ($0 ~ /[0-9]/) print "contradiction: digit on a non-entry line: " $0 }
' "$committed" >"$contradictions"

# Every id the committed file cites must still exist in the corpus.
awk '/^- [0-9]+ / { print $2 }' "$committed" | sort -u >"$old_ids"
awk '/^- [0-9]+ / { print $2 }' "$generated" | sort -u >"$new_ids"
comm -23 "$old_ids" "$new_ids" | while IFS= read -r id; do
    echo "contradiction: id $id is not in the corpus" >>"$contradictions"
done

if [ -s "$contradictions" ]; then
    cat "$contradictions" >&2
    echo "error: $committed contradicts the design corpus" >&2
    exit 1
fi

if cmp -s "$committed" "$generated"; then
    echo "design-index-check: $committed is up to date"
    exit 0
fi

echo "warning: $committed is behind the design corpus (only additions, retitles or status changes) — run 'just design-index' to refresh"
[ "$strict" -eq 1 ] && exit 1
exit 0
