#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Build the slint-viewer Android App Bundle for Play Store upload.
#
# SLINT_BUILD_NUMBER (the Play Store versionCode) defaults to the git commit
# count; override as an env var.
#
# Signing (omit all three for an unsigned local bundle):
#   ANDROID_KEYSTORE_PATH      upload keystore path
#   ANDROID_KEYSTORE_PASSWORD  upload keystore password, also unlocks the key
#   ANDROID_KEYSTORE_ALIAS     alias of the signing key in the keystore
#
# Requires: what build-native.sh needs; gradle 8.11.1+ on PATH (AGP 8.10).
#
# Output: app/build/outputs/bundle/release/slint-viewer.aab

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/../../.." && pwd)"

# Check the signing setup before the long Rust build so it fails fast.
if [ -n "${ANDROID_KEYSTORE_PATH:-}" ]; then
    [ -n "${ANDROID_KEYSTORE_PASSWORD:-}" ] || { echo "set ANDROID_KEYSTORE_PASSWORD" >&2; exit 1; }
    [ -n "${ANDROID_KEYSTORE_ALIAS:-}" ] || { echo "set ANDROID_KEYSTORE_ALIAS" >&2; exit 1; }
fi

: "${SLINT_BUILD_NUMBER:=$(git -C "$REPO_ROOT" rev-list --count HEAD)}"
export SLINT_BUILD_NUMBER

"$PROJECT_DIR/build-native.sh"

cd "$PROJECT_DIR"
gradle --no-daemon bundleRelease

# Gradle names the bundle app-release.aab; rename it to the app.
BUNDLE_DIR="$PROJECT_DIR/app/build/outputs/bundle/release"
mv -f "$BUNDLE_DIR/app-release.aab" "$BUNDLE_DIR/slint-viewer.aab"

echo "AAB built at: $BUNDLE_DIR/slint-viewer.aab"
