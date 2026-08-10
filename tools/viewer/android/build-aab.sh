#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore xxxhdpi jniLibs
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
# Requires: cargo-ndk; Android SDK + NDK with ANDROID_HOME / ANDROID_NDK_HOME
# set; gradle 8.11.1+ on PATH (AGP 8.10); JDK 17; the three Android rust targets
# (rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android).
#
# Output: app/build/outputs/bundle/release/slint-viewer.aab

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/../../.." && pwd)"

[ -n "${ANDROID_HOME:-}" ] || { echo "set ANDROID_HOME to your Android SDK" >&2; exit 1; }

# Check the signing setup before the long Rust build so it fails fast.
if [ -n "${ANDROID_KEYSTORE_PATH:-}" ]; then
    [ -n "${ANDROID_KEYSTORE_PASSWORD:-}" ] || { echo "set ANDROID_KEYSTORE_PASSWORD" >&2; exit 1; }
    [ -n "${ANDROID_KEYSTORE_ALIAS:-}" ] || { echo "set ANDROID_KEYSTORE_ALIAS" >&2; exit 1; }
fi

: "${SLINT_BUILD_NUMBER:=$(git -C "$REPO_ROOT" rev-list --count HEAD)}"
export SLINT_BUILD_NUMBER

# cargo-ndk sets ANDROID_PLATFORM to the NDK API level, which the android-build
# crate then reads as the SDK platform string and tries to load from
# platforms/android-<NDK level>/android.jar. Pin ANDROID_JAR to the highest
# installed platform to bypass.
if [ -z "${ANDROID_JAR:-}" ]; then
    ANDROID_JAR=$(ls "$ANDROID_HOME"/platforms/*/android.jar 2>/dev/null | sort -V | tail -1 || true)
    [ -n "$ANDROID_JAR" ] || { echo "no android.jar under $ANDROID_HOME/platforms" >&2; exit 1; }
    export ANDROID_JAR
fi

# F-Droid reproducibility hints:
# - SOURCE_DATE_EPOCH stamps AGP zip entries.
# - --remap-path-prefix scrubs source paths from rustc-embedded strings.
# - --locked pins Cargo.lock.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git -C "$REPO_ROOT" log -1 --pretty=%ct)}"
CARGO_HOME_PATH="${CARGO_HOME:-$HOME/.cargo}"
REMAP_FLAGS="--remap-path-prefix=$CARGO_HOME_PATH=/cargo --remap-path-prefix=$REPO_ROOT=/build"
export CARGO_BUILD_RUSTFLAGS="${CARGO_BUILD_RUSTFLAGS:-} $REMAP_FLAGS"

# Render the launcher icons so the AAB carries the current logo. Bail if the
# renderer succeeded but wrote empty PNGs.
"$REPO_ROOT/scripts/render_android_app_icon.bash"
RES_DIR="$PROJECT_DIR/app/src/main/res/mipmap-xxxhdpi"
[ -s "$RES_DIR/ic_launcher.png" ] && [ -s "$RES_DIR/ic_launcher_foreground.png" ] \
    || { echo "icon render produced empty PNGs" >&2; exit 1; }

JNI_DIR="$PROJECT_DIR/app/src/main/jniLibs"
rm -rf "$JNI_DIR"
mkdir -p "$JNI_DIR"

cd "$REPO_ROOT"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 --platform 26 -o "$JNI_DIR" \
    build --release --locked -p slint-viewer --lib --features remote

cd "$PROJECT_DIR"
gradle --no-daemon bundleRelease

# Gradle names the bundle app-release.aab; rename it to the app.
BUNDLE_DIR="$PROJECT_DIR/app/build/outputs/bundle/release"
mv -f "$BUNDLE_DIR/app-release.aab" "$BUNDLE_DIR/slint-viewer.aab"

echo "AAB built at: $BUNDLE_DIR/slint-viewer.aab"
