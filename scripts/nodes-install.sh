#!/bin/sh
# Install the pinned `nodes` release binary (github.com/ZurNetwork/nodes — the
# NODE.json normalizer + lookup tool) into ${CARGO_HOME:-$HOME/.cargo}/bin.
#
#   scripts/nodes-install.sh vX.Y.Z      (the Justfile passes NODES_VERSION)
#
# Idempotent: exits at once when the installed binary already reports the pinned
# version. Prefers the prebuilt archive published for this OS/arch by the tool's
# release workflow (checksum-verified); falls back to `cargo install --git
# --locked --tag` when no archive fits this machine. The pin lives in ONE place,
# the Justfile's NODES_VERSION: the NODE.json files and the tool's schema move
# in lockstep on it.
set -eu

TAG="${1:?usage: nodes-install.sh vX.Y.Z}"
REPO="https://github.com/ZurNetwork/nodes"
BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
WANT="${TAG#v}"

if command -v nodes >/dev/null 2>&1; then
    have="$(nodes --version 2>/dev/null | awk '{print $2}')"
    if [ "$have" = "$WANT" ]; then
        echo "nodes-install: nodes $have already installed."
        exit 0
    fi
fi

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)  target=x86_64-unknown-linux-gnu ;;
    Linux-aarch64) target=aarch64-unknown-linux-gnu ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Darwin-arm64)  target=aarch64-apple-darwin ;;
    *)             target= ;;
esac

mkdir -p "$BIN_DIR"
if [ -n "$target" ]; then
    archive="nodes-$TAG-$target.tar.gz"
    url="$REPO/releases/download/$TAG/$archive"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    echo "nodes-install: fetching $url"
    if curl -fsSL "$url" -o "$tmp/$archive" && curl -fsSL "$url.sha256" -o "$tmp/$archive.sha256"; then
        if command -v shasum >/dev/null 2>&1; then
            (cd "$tmp" && shasum -a 256 -c "$archive.sha256" >/dev/null)
        else
            (cd "$tmp" && sha256sum -c "$archive.sha256" >/dev/null)
        fi
        tar -xzf "$tmp/$archive" -C "$BIN_DIR" nodes
        chmod +x "$BIN_DIR/nodes"
        echo "nodes-install: installed nodes $WANT into $BIN_DIR."
        exit 0
    fi
    echo "nodes-install: no prebuilt archive for $target at $TAG — building from source."
fi

command -v cargo >/dev/null 2>&1 || { echo "nodes-install: cargo not found (install the Rust toolchain)"; exit 1; }
cargo install --git "$REPO" --locked --tag "$TAG" nodes
