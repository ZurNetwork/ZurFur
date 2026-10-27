#!/usr/bin/env bash
# The gallery's screenshot run, in the pinned Playwright image with no network:
#   gallery-shots.sh run [--update] [--ci]
# Every story and docs page in both skins, checked against (or, with --update,
# written into) frontend/ui/shots/__screenshots__. Run through `just
# gallery-shots-run`, which exports PLAYWRIGHT_IMAGE.
#
# Before the container starts it refuses (exit 2, one line each): a bad
# argument, a pin off the pattern, a tag version other than the one locked for
# @playwright/test, playwright and playwright-core and installed, a missing
# gallery build or install, no Podman or Docker, an image whose RepoDigests lack
# the pinned digest. The container gets read-only mounts of the build, the
# shots folder, ui's package.json and frontend/node_modules, and read-write
# mounts of a fresh, empty shots/output (and, with --update, the references);
# nothing else. After it exits, --update walks the references without following
# links and refuses links, special files, non-PNG files and oversized ones.
# Exits with Playwright's code, or 1 when the walk found anything.
set -euo pipefail

usage='usage: gallery-shots.sh run [--update] [--ci]'

refuse() {
    echo "gallery-shots: $1" >&2
    exit 2
}

refuse_usage() {
    echo "gallery-shots: $1" >&2
    echo "$usage" >&2
    exit 2
}

# --- arguments ---------------------------------------------------------------------

if [ "$#" -eq 0 ]; then
    refuse_usage "no command"
fi
if [ "$1" != run ]; then
    refuse_usage "unknown command '$1'"
fi
shift
update=0
ci=0
for arg in "$@"; do
    case "$arg" in
        --update)
            [ "$update" = 0 ] || refuse_usage "--update given twice"
            update=1
            ;;
        --ci)
            [ "$ci" = 0 ] || refuse_usage "--ci given twice"
            ci=1
            ;;
        *) refuse_usage "unknown argument '$arg'" ;;
    esac
done
if [ "$update" = 1 ] && [ "$ci" = 1 ]; then
    refuse_usage "--update and --ci together: CI never writes references"
fi

self="${BASH_SOURCE[0]}"
case "$self" in
    */*) self_dir="${self%/*}" ;;
    *) self_dir=. ;;
esac
root="$(cd -- "$self_dir/.." && pwd -P)"
case "$root" in
    *:* | *,* | *$'\n'*) refuse "the repository path '$root' holds a ':', ',' or newline, which a -v mount can't carry" ;;
esac

# --- the pin -----------------------------------------------------------------------

pin="${PLAYWRIGHT_IMAGE-}"
if [ -z "$pin" ]; then
    refuse "PLAYWRIGHT_IMAGE is not set: run through \`just gallery-shots\` or \`just gallery-shots-run\`"
fi
pin_pattern='^mcr\.microsoft\.com/playwright:v([0-9]+\.[0-9]+\.[0-9]+)-noble@sha256:([0-9a-f]{64})$'
if ! [[ "$pin" =~ $pin_pattern ]]; then
    refuse "PLAYWRIGHT_IMAGE '$pin' is not mcr.microsoft.com/playwright:v<x.y.z>-noble@sha256:<64 hex>"
fi
tag_version="${BASH_REMATCH[1]}"
digest="${BASH_REMATCH[2]}"
image_ref="mcr.microsoft.com/playwright@sha256:$digest"

# --- the build and the install -----------------------------------------------------

ui="$root/frontend/ui"
build="$ui/storybook-static"
shots="$ui/shots"
references="$shots/__screenshots__"
output="$shots/output"
modules="$root/frontend/node_modules"
installed_manifest="$modules/@playwright/test/package.json"

if [ ! -f "$build/index.json" ]; then
    refuse "no frontend/ui/storybook-static/index.json: run \`just gallery-build\` first"
fi
if [ ! -f "$installed_manifest" ]; then
    refuse "no frontend/node_modules/@playwright/test/package.json: run \`just frontend-install\` first"
fi
if [ -L "$shots" ] || [ ! -d "$shots" ]; then
    refuse "frontend/ui/shots is not a folder"
fi
if [ -L "$ui/package.json" ] || [ ! -f "$ui/package.json" ]; then
    refuse "frontend/ui/package.json is not a regular file"
fi

# --- the tag against the lock and the install ---------------------------------------

# locked_versions <name>: each version frontend/yarn.lock locks <name> at, one per line.
locked_versions() {
    awk -v want="$1" '
        /^[[:space:]]*$/ { inside = 0; next }
        /^[^[:space:]#]/ {
            inside = 0
            header = $0
            sub(/:[[:space:]]*$/, "", header)
            count = split(header, specs, /,[[:space:]]*/)
            for (i = 1; i <= count; i++) {
                spec = specs[i]
                gsub(/^"|"$/, "", spec)
                at = 0
                for (j = length(spec); j > 1; j--) {
                    if (substr(spec, j, 1) == "@") { at = j; break }
                }
                if (at > 0 && substr(spec, 1, at - 1) == want) inside = 1
            }
            next
        }
        inside && /^[[:space:]]+version[[:space:]]/ {
            version = $2
            gsub(/"/, "", version)
            print version
        }
    ' "$root/frontend/yarn.lock" | sort -u
}

if [ ! -f "$root/frontend/yarn.lock" ]; then
    refuse "no frontend/yarn.lock"
fi
for package in @playwright/test playwright playwright-core; do
    versions="$(locked_versions "$package")"
    if [ -z "$versions" ]; then
        refuse "frontend/yarn.lock locks no $package"
    fi
    if [ "$(printf '%s\n' "$versions" | wc -l)" -ne 1 ]; then
        refuse "frontend/yarn.lock locks $package at more than one version: $(printf '%s' "$versions" | tr '\n' ' ')"
    fi
    if [ "$versions" != "$tag_version" ]; then
        refuse "frontend/yarn.lock locks $package at $versions, but PLAYWRIGHT_IMAGE's tag is v$tag_version"
    fi
done
installed_version="$(awk '
    match($0, /^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"[^"]*"/) {
        line = substr($0, RSTART, RLENGTH)
        sub(/^[^:]*:[[:space:]]*"/, "", line)
        sub(/"$/, "", line)
        print line
        exit
    }
' "$installed_manifest")"
if [ "$installed_version" != "$tag_version" ]; then
    refuse "frontend/node_modules has @playwright/test ${installed_version:-of no version}, but PLAYWRIGHT_IMAGE's tag is v$tag_version: run \`just frontend-install\`"
fi

# --- the runtime and the image ------------------------------------------------------

if command -v podman >/dev/null 2>&1; then
    runtime=podman
    flavour=podman
elif command -v docker >/dev/null 2>&1; then
    runtime=docker
    flavour=docker
    if docker --version 2>/dev/null | grep -qi podman; then
        flavour=podman
    fi
else
    refuse "no container runtime: install Podman or Docker"
fi

if ! "$runtime" image inspect "$image_ref" >/dev/null 2>&1; then
    echo "gallery-shots: pulling $image_ref" >&2
    if ! "$runtime" pull --platform linux/amd64 "$image_ref"; then
        refuse "could not pull $image_ref"
    fi
fi
if ! repo_digests="$("$runtime" image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' "$image_ref")"; then
    refuse "could not inspect $image_ref"
fi
if ! printf '%s\n' "$repo_digests" | grep -qxF -- "$image_ref"; then
    refuse "the image's RepoDigests lack $image_ref"
fi

# --- the folders ----------------------------------------------------------------------

if [ "$update" = 1 ]; then
    if [ -L "$references" ]; then
        refuse "frontend/ui/shots/__screenshots__ is a symlink"
    fi
    if [ -e "$references" ] && [ ! -d "$references" ]; then
        refuse "frontend/ui/shots/__screenshots__ is not a folder"
    fi
    mkdir -p -- "$references"
fi
# A fresh, empty output folder: whatever a run left there (links included) goes.
rm -rf -- "$output"
mkdir -- "$output"

# --- the run ------------------------------------------------------------------------

run_flags=(run --rm --platform linux/amd64 --network none --read-only --tmpfs /tmp:rw,nosuid,size=2g --cap-drop ALL --security-opt no-new-privileges --init)
if [ "$flavour" = podman ]; then
    run_flags+=(--userns=keep-id --http-proxy=false)
else
    run_flags+=(--user "$(id -u):$(id -g)")
fi
mounts=(
    -v "$build:/work/frontend/ui/storybook-static:ro"
    -v "$shots:/work/frontend/ui/shots:ro"
    -v "$ui/package.json:/work/frontend/ui/package.json:ro"
    -v "$modules:/work/frontend/node_modules:ro"
    -v "$output:/work/frontend/ui/shots/output:rw"
)
if [ "$update" = 1 ]; then
    mounts+=(-v "$references:/work/frontend/ui/shots/__screenshots__:rw")
fi
environment=(-e HOME=/tmp -e TZ=UTC -e "ZURFUR_GALLERY_IMAGE=$pin")
if [ "$ci" = 1 ]; then
    environment+=(-e CI=1)
fi
command=(node /work/frontend/node_modules/@playwright/test/cli.js test --config shots/playwright.config.ts)
if [ "$update" = 1 ]; then
    command+=(--update-snapshots=changed --project=stories)
else
    command+=(--update-snapshots=none)
fi

set +e
"$runtime" "${run_flags[@]}" "${mounts[@]}" -w /work/frontend/ui "${environment[@]}" "$image_ref" "${command[@]}"
status=$?
set -e

# --- the reference walk -------------------------------------------------------------

findings=0
finding() {
    echo "gallery-shots: refused ${1#"$root/"}: $2" >&2
    findings=$((findings + 1))
}

# check_png <path>: the PNG signature, then an IHDR chunk of at most 4096 px a side.
check_png() {
    local head
    head="$(od -An -v -tu1 -N24 -- "$1" | tr -s ' \n' ' ')"
    # shellcheck disable=SC2086
    set -- "$1" $head
    if [ "$#" -lt 25 ]; then
        finding "$1" "shorter than a PNG header"
        return
    fi
    local path="$1"
    shift
    if [ "$1 $2 $3 $4 $5 $6 $7 $8" != "137 80 78 71 13 10 26 10" ]; then
        finding "$path" "not a PNG (wrong signature)"
        return
    fi
    if [ "$9 ${10} ${11} ${12} ${13} ${14} ${15} ${16}" != "0 0 0 13 73 72 68 82" ]; then
        finding "$path" "not a PNG (no IHDR chunk first)"
        return
    fi
    local width=$(((${17} << 24) + (${18} << 16) + (${19} << 8) + ${20}))
    local height=$(((${21} << 24) + (${22} << 16) + (${23} << 8) + ${24}))
    if [ "$width" -gt 4096 ] || [ "$height" -gt 4096 ]; then
        finding "$path" "${width}x${height} px, over 4096 px a side"
    fi
}

# walk <folder>: every entry below it, links never followed.
walk() {
    local entry size
    if [ ! -r "$1" ] || [ ! -x "$1" ]; then
        finding "$1" "an unreadable folder"
        return
    fi
    for entry in "$1"/* "$1"/.*; do
        case "${entry##*/}" in . | ..) continue ;; esac
        if [ -L "$entry" ]; then
            finding "$entry" "a symlink"
        elif [ -d "$entry" ]; then
            walk "$entry"
        elif [ ! -f "$entry" ]; then
            finding "$entry" "not a regular file or folder"
        elif [ "${entry%.png}" = "$entry" ]; then
            finding "$entry" "not named *.png"
        else
            size="$(wc -c <"$entry")"
            if [ "$size" -gt 1048576 ]; then
                finding "$entry" "$size bytes, over 1 MiB"
            else
                check_png "$entry"
            fi
        fi
    done
}

if [ "$update" = 1 ]; then
    shopt -s nullglob
    walk "$references"
    if [ "$findings" -gt 0 ]; then
        echo "gallery-shots: $findings finding(s) in frontend/ui/shots/__screenshots__" >&2
        exit 1
    fi
fi
exit "$status"
