#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$SCRIPT_DIR/helpers.sh"

usage() {
    echo "Usage: $0 --bin <binary name> [--profile <profile>] [--] [cargo build args...]" >&2
}

if [ "$#" -lt 2 ]; then
    usage
    exit 2
fi

CARGO_TARGET_NAME=""
CARGO_PROFILE=dev

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin)
            CARGO_TARGET_NAME="$2"
            shift
            shift
            ;;
        --profile)
            CARGO_PROFILE="$2"
            shift
            shift
            ;;
        --*)
            break
            ;;
        *)
            usage
            exit 2
            ;;
    esac
done

if [ -z "${CARGO_TARGET_NAME}" ]; then
    usage
    exit 2
fi

if [ -z "${TARGET_BUILD_DIR:-}" ] || [ -z "${EXECUTABLE_PATH:-}" ]; then
    echo "error: TARGET_BUILD_DIR and EXECUTABLE_PATH must be provided by Xcode" >&2
    exit 1
fi

export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH:$HOME/.cargo/bin"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ "${CARGO_PROFILE}" = "dev" ]; then
    CARGO_PROFILE_DIR=debug
else
    CARGO_PROFILE_DIR="$CARGO_PROFILE"
fi

CARGO_TIMINGS_ARGS=()
if [ "${MACOS_CARGO_TIMINGS:-0}" != "0" ]; then
    CARGO_TIMINGS_ARGS=(--timings)
fi

TARGET_DIR_NAME="${MACOS_CARGO_TARGET_DIR_NAME:-${PRODUCT_BUNDLE_IDENTIFIER:-$CARGO_TARGET_NAME}}"
TARGET_DIR_NAME="$(printf "%s" "$TARGET_DIR_NAME" | tr -c 'A-Za-z0-9_.-' '-')"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/xcode-cargo/$TARGET_DIR_NAME}"

RUST_TARGET=aarch64-apple-darwin

echo "::group::Cargo build $CARGO_TARGET_NAME for $RUST_TARGET"
log "cargo build start: $RUST_TARGET"

env RUSTFLAGS='-Clink-args=-Wl,-rpath,@loader_path/../Frameworks' \
    cargo build \
        ${CARGO_TIMINGS_ARGS[@]+"${CARGO_TIMINGS_ARGS[@]}"} \
        --target "$RUST_TARGET" \
        --bin "$CARGO_TARGET_NAME" \
        --profile "$CARGO_PROFILE" \
        "$@"
EXECUTABLE="$CARGO_TARGET_DIR/$RUST_TARGET/$CARGO_PROFILE_DIR/$CARGO_TARGET_NAME"

mkdir -p "$(dirname "$TARGET_BUILD_DIR/$EXECUTABLE_PATH")"
rm -f "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
echo "Copying $EXECUTABLE to $TARGET_BUILD_DIR/$EXECUTABLE_PATH"
cp "$EXECUTABLE" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
chmod +x "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"

log "cargo build finished: $RUST_TARGET"
echo "::endgroup::"
