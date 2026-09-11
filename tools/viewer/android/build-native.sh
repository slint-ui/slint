#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore jniLibs ffile CXXFLAGS
#
# Produce what gradle needs that isn't checked in: the launcher icons and the
# slint-viewer native libraries under app/src/main/jniLibs.
#
# Usage: build-native.sh [ABI...]     default: arm64-v8a armeabi-v7a x86_64
#
# F-Droid ships our signed APK only if its own rebuild, which runs this
# script too, is byte-identical. Keep everything that influences the
# binaries in here rather than in the callers.
#
# Requires: rustup, cargo-ndk, resvg; an Android SDK with the platform and
# build-tools of app/build.gradle.kts, NDK r27, ANDROID_HOME and
# ANDROID_NDK_HOME set; JDK 21 like F-Droid; clang, python3 and ninja for Skia.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/../../.." && pwd)"

ABIS=("$@")
[ ${#ABIS[@]} -gt 0 ] || ABIS=(arm64-v8a armeabi-v7a x86_64)

[ -n "${ANDROID_HOME:-}" ] || { echo "set ANDROID_HOME to your Android SDK" >&2; exit 1; }
[ -n "${ANDROID_NDK_HOME:-}" ] || { echo "set ANDROID_NDK_HOME to NDK r27" >&2; exit 1; }

gradle_setting() { sed -n "s/^ *$1 = \"\\?\([0-9.]*\)\"\\?\$/\\1/p" "$PROJECT_DIR/app/build.gradle.kts"; }

# The android-activity backend compiles and embeds a Java helper. Compile it
# against the gradle project's platform and build-tools rather than the ones
# cargo-ndk and the machine happen to suggest.
export ANDROID_JAR="$ANDROID_HOME/platforms/android-$(gradle_setting compileSdk)/android.jar"
export ANDROID_BUILD_TOOLS_VERSION="$(gradle_setting buildToolsVersion)"

# F-Droid forbids the prebuilt Skia rust-skia downloads by default; compile
# it with the NDK, which Skia's build finds through ANDROID_NDK.
export FORCE_SKIA_BUILD=1
export ANDROID_NDK="$ANDROID_NDK_HOME"

# Skia's build refers to its sources in the cargo registry by a path relative
# to the build directory, and that path ends up in the binary. A cargo home
# inside the target directory keeps it the same on every machine; the crate
# cache is shared with the regular cargo home to spare downloads.
DEFAULT_CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export CARGO_HOME="$REPO_ROOT/target/cargo-home"
mkdir -p "$DEFAULT_CARGO_HOME/registry/cache" "$CARGO_HOME/registry"
ln -sfn "$DEFAULT_CARGO_HOME/registry/cache" "$CARGO_HOME/registry/cache"

# Keep the checkout and toolchain locations out of the binaries.
export RUSTFLAGS="${RUSTFLAGS:-} \
    --remap-path-prefix=$REPO_ROOT=/build \
    --remap-path-prefix=$(rustc --print sysroot)=/rustc-sysroot"
PREFIX_MAP="-ffile-prefix-map=$REPO_ROOT=/build"
export CFLAGS="${CFLAGS:-} $PREFIX_MAP"
export CXXFLAGS="${CXXFLAGS:-} $PREFIX_MAP"

"$REPO_ROOT/scripts/render_android_app_icon.bash"

JNI_DIR="$PROJECT_DIR/app/src/main/jniLibs"
rm -rf "$JNI_DIR"
mkdir -p "$JNI_DIR"

# rust-toolchain.toml applies to cargo runs from this directory; the fallback
# installs its toolchain on rustup 1.28, which doesn't do so on demand.
cd "$PROJECT_DIR"
rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install --no-self-update >/dev/null
cargo ndk --manifest-path "$REPO_ROOT/Cargo.toml" "${ABIS[@]/#/--target=}" \
    --platform "$(gradle_setting minSdk)" -o "$JNI_DIR" \
    build --profile package-release --locked -p slint-viewer --lib --features remote
