#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Build the slint-viewer Android App Bundle for Play Store upload, plus one
# APK per ABI for GitHub releases and the version file F-Droid polls.
#
# SLINT_BUILD_NUMBER (the Play Store versionCode of the bundle) defaults to
# the git commit count; override as an env var. The APKs derive their
# versionCode from the version, see app/build.gradle.kts.
#
# Signing (omit all three for unsigned outputs):
#   ANDROID_KEYSTORE_PATH      upload keystore path
#   ANDROID_KEYSTORE_PASSWORD  upload keystore password, also unlocks the key
#   ANDROID_KEYSTORE_ALIAS     alias of the signing key in the keystore
#
# Requires: what build-native.sh needs; gradle 8.11.1+ on PATH (AGP 8.10).
#
# Output: app/build/outputs/bundle/release/slint-viewer.aab
#         app/build/outputs/apk/release/slint-viewer-<abi>.apk
#         app/build/outputs/apk/release/slint-viewer-android-version.txt

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
gradle --no-daemon bundleRelease assembleRelease

# Gradle names the bundle app-release.aab; rename it to the app.
BUNDLE_DIR="$PROJECT_DIR/app/build/outputs/bundle/release"
mv -f "$BUNDLE_DIR/app-release.aab" "$BUNDLE_DIR/slint-viewer.aab"

# Name the per-ABI APKs after their ABI, and write the release version code
# (the per-ABI codes without their ABI digit) and name for F-Droid's update
# check. AGP lists all of it in output-metadata.json.
APK_DIR="$PROJECT_DIR/app/build/outputs/apk/release"
python3 - "$APK_DIR" <<'EOF'
import json, os, sys
apk_dir = sys.argv[1]
outputs = json.load(open(os.path.join(apk_dir, "output-metadata.json")))["elements"]
for output in outputs:
    abi = next(f["value"] for f in output["filters"] if f["filterType"] == "ABI")
    os.replace(os.path.join(apk_dir, output["outputFile"]), os.path.join(apk_dir, f"slint-viewer-{abi}.apk"))
with open(os.path.join(apk_dir, "slint-viewer-android-version.txt"), "w") as f:
    f.write(f"versionCode={outputs[0]['versionCode'] // 10}\nversionName={outputs[0]['versionName']}\n")
EOF

ls "$BUNDLE_DIR/slint-viewer.aab" "$APK_DIR"/slint-viewer-*
