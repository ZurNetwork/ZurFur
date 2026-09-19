#!/bin/sh
# Shared helpers for the Claude Code hook scripts (scripts/uow-status.sh,
# scripts/hooks/*.sh): the ZURFUR_HOOKS_OFF kill switch, the untracked
# .claude/hooks.off escape hatch, and the text-safety helpers every hook
# applies to data it injects into a model's context. POSIX sh — sourced
# from both sh and bash.

# hooks_are_off <name> <project_dir> returns success when hooks named <name>
# are disabled: an untracked "<project_dir>/.claude/hooks.off" file is
# present, or ZURFUR_HOOKS_OFF (a comma/space separated list) contains "all"
# or <name> exactly.
hooks_are_off() {
    _hooks_off_name="$1"
    _hooks_off_project_dir="$2"
    [ -f "$_hooks_off_project_dir/.claude/hooks.off" ] && return 0
    _hooks_off_list="${ZURFUR_HOOKS_OFF:-}"
    [ -z "$_hooks_off_list" ] && return 1
    _hooks_off_saved_ifs="$IFS"
    IFS=', '
    for _hooks_off_tok in $_hooks_off_list; do
        if [ "$_hooks_off_tok" = "all" ] || [ "$_hooks_off_tok" = "$_hooks_off_name" ]; then
            IFS="$_hooks_off_saved_ifs"
            return 0
        fi
    done
    IFS="$_hooks_off_saved_ifs"
    return 1
}

# neutralize <text> — prints <text> with CR/LF/TAB collapsed to a single
# space, every other control character stripped, and '<'/'>' removed, so
# free-text pulled from a ledger or a NODE.json can't fake a newline-
# delimited record, a control sequence, or tag-shaped instruction text once
# it lands in a model's context as injected data.
neutralize() {
    printf '%s' "$1" | tr '\n\r\t' '   ' | tr -d '[:cntrl:]' | tr -s ' ' | tr -d '<>'
}

# has_control_char <text> — true when <text> contains any control byte
# (including newline/CR), which must never be used unsanitized as a
# dedupe-state key or line in a flat text file.
has_control_char() {
    _hcc_stripped="$(printf '%s' "$1" | tr -d '[:cntrl:]')"
    [ "$_hcc_stripped" != "$1" ]
}

# utf8_safe_head <n> — reads stdin, prints at most <n> bytes, backed off to
# the nearest valid UTF-8 boundary. A plain `head -c N` can cut in the
# middle of a multi-byte character (this text is design-corpus/ledger
# prose — em dashes are routine); a downstream UTF-8-strict reader (jq,
# building the hookSpecificOutput JSON) then replaces that invalid tail
# with U+FFFD, which can change the byte count the caller computed by a
# few bytes — silently breaking an exact byte cap. No-op when `iconv`
# isn't on PATH: `iconv` is a POSIX-specified utility, so this is a
# best-effort second line of defense, not the primary size guarantee.
# `iconv -c` exits 1 whenever it actually drops something — the expected,
# common case right here — so its status is discarded; only its (valid)
# stdout matters.
utf8_safe_head() {
    if command -v iconv >/dev/null 2>&1; then
        head -c "$1" | { iconv -f UTF-8 -t UTF-8 -c 2>/dev/null || true; }
    else
        head -c "$1"
    fi
}

# cap_bytes <text> <n> — prints <text> truncated to at most <n> bytes,
# appending "..." when it was cut. Byte-bounded (not codepoint-aware) on
# purpose: the guarantee that matters here is the hard size cap, not
# perfectly preserving a multi-byte character at the cut point — hence
# utf8_safe_head backing off to a valid boundary rather than an exact one.
cap_bytes() {
    _cap_text="$1"
    _cap_n="$2"
    _cap_len="$(printf '%s' "$_cap_text" | wc -c)"
    if [ "$_cap_len" -le "$_cap_n" ]; then
        printf '%s' "$_cap_text"
        return 0
    fi
    if [ "$_cap_n" -gt 3 ]; then
        printf '%s' "$_cap_text" | utf8_safe_head $((_cap_n - 3))
        printf '...'
    else
        printf '%s' "$_cap_text" | utf8_safe_head "$_cap_n"
    fi
}

# safe_state_dir <dir> — creates <dir> (and its parents) with mode 700 via
# umask, atomically (no separate chmod, so no window where it's briefly
# world-readable), then refuses it — printing nothing, returning failure —
# if it is a symlink or not owned by the current user. Callers must treat
# a failure as "silent no-op", never as a fatal error.
safe_state_dir() {
    _ssd_dir="$1"
    if [ -e "$_ssd_dir" ] && [ ! -d "$_ssd_dir" ]; then
        return 1
    fi
    if [ -L "$_ssd_dir" ]; then
        return 1
    fi
    (umask 077 && mkdir -p "$_ssd_dir") 2>/dev/null || true
    [ -d "$_ssd_dir" ] || return 1
    [ -L "$_ssd_dir" ] && return 1
    _ssd_owner="$(find "$_ssd_dir" -maxdepth 0 -printf '%U' 2>/dev/null || stat -c '%u' "$_ssd_dir" 2>/dev/null || stat -f '%u' "$_ssd_dir" 2>/dev/null || true)"
    if [ -n "$_ssd_owner" ] && [ "$_ssd_owner" != "$(id -u)" ]; then
        return 1
    fi
    return 0
}

# bounded_append <file> <line> <max_lines> — appends <line> to <file>
# unless it already has at least <max_lines> lines, so a session that
# touches an unbounded number of distinct files/nodes can't grow the
# dedupe-state file without limit. Silent either way.
bounded_append() {
    _bnd_file="$1"
    _bnd_line="$2"
    _bnd_max="$3"
    _bnd_count="$(wc -l <"$_bnd_file" 2>/dev/null || echo 0)"
    [ "$_bnd_count" -ge "$_bnd_max" ] && return 0
    printf '%s\n' "$_bnd_line" >>"$_bnd_file"
}
