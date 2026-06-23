#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

usage() {
    echo "Usage: $0 (--bin|--example) <cargo-target-name> [cargo build args...]" >&2
}

if [ "$#" -lt 2 ]; then
    usage
    exit 2
fi

CARGO_TARGET_KIND="$1"
CARGO_TARGET_NAME="$2"
shift 2

case "$CARGO_TARGET_KIND" in
    --bin | --example)
        ;;
    *)
        usage
        exit 2
        ;;
esac

if [ -z "${TARGET_BUILD_DIR:-}" ] || [ -z "${EXECUTABLE_PATH:-}" ]; then
    echo "error: TARGET_BUILD_DIR and EXECUTABLE_PATH must be provided by Xcode" >&2
    exit 1
fi

export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH:$HOME/.cargo/bin"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ "${CONFIGURATION:-Debug}" = "Debug" ]; then
    CARGO_PROFILE=debug
    CARGO_PROFILE_ARGS=()
else
    CARGO_PROFILE=release
    CARGO_PROFILE_ARGS=(--release)
fi

TARGET_DIR_NAME="${MACOS_CARGO_TARGET_DIR_NAME:-${PRODUCT_BUNDLE_IDENTIFIER:-$CARGO_TARGET_NAME}}"
TARGET_DIR_NAME="$(printf "%s" "$TARGET_DIR_NAME" | tr -c 'A-Za-z0-9_.-' '-')"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/xcode-cargo/$TARGET_DIR_NAME}"

ARCHS="${ARCHS:-$(uname -m)}"
EXECUTABLES=()

for arch in $ARCHS; do
    case "$arch" in
        arm64)
            RUST_TARGET=aarch64-apple-darwin
            ;;
        x86_64)
            RUST_TARGET=x86_64-apple-darwin
            ;;
        *)
            echo "error: unsupported macOS architecture: $arch" >&2
            exit 1
            ;;
    esac

    cargo build \
        "${CARGO_PROFILE_ARGS[@]}" \
        --target "$RUST_TARGET" \
        "$CARGO_TARGET_KIND" "$CARGO_TARGET_NAME" \
        "$@"

    if [ "$CARGO_TARGET_KIND" = "--example" ]; then
        EXECUTABLES+=("$CARGO_TARGET_DIR/$RUST_TARGET/$CARGO_PROFILE/examples/$CARGO_TARGET_NAME")
    else
        EXECUTABLES+=("$CARGO_TARGET_DIR/$RUST_TARGET/$CARGO_PROFILE/$CARGO_TARGET_NAME")
    fi
done

mkdir -p "$(dirname "$TARGET_BUILD_DIR/$EXECUTABLE_PATH")"
rm -f "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"

if [ "${#EXECUTABLES[@]}" -eq 1 ]; then
    cp "${EXECUTABLES[0]}" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
else
    # Source: lipo creates a universal file from multiple architecture-specific
    # Mach-O inputs: https://keith.github.io/xcode-man-pages/lipo.1.html
    lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" "${EXECUTABLES[@]}"
fi

chmod +x "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
