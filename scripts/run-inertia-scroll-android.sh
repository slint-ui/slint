#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euo pipefail

PACKAGE="${INERTIA_PROBE_PACKAGE:-dev.slint.examples.inertiascrollprobe}"
ACTIVITY="${INERTIA_PROBE_ACTIVITY:-android.app.NativeActivity}"
TARGET="${ANDROID_TARGET:-aarch64-linux-android}"
ADB="${ADB:-adb}"
DEFAULT_ANDROID_HOME="/opt/homebrew/share/android-commandlinetools"

command -v cargo-apk >/dev/null || {
    echo "cargo-apk is required. Install it with: cargo install cargo-apk" >&2
    exit 1
}

command -v "$ADB" >/dev/null || {
    echo "adb is required. Put Android platform-tools on PATH or set ADB=/path/to/adb" >&2
    exit 1
}

if [ -z "${ANDROID_HOME:-}" ] && [ -d "$DEFAULT_ANDROID_HOME" ]; then
    export ANDROID_HOME="$DEFAULT_ANDROID_HOME"
fi

if [ -z "${ANDROID_HOME:-}" ]; then
    echo "ANDROID_HOME must point to an Android SDK installation" >&2
    exit 1
fi

if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    ANDROID_NDK_ROOT="$(find "$ANDROID_HOME/ndk" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | tail -1)"
    export ANDROID_NDK_ROOT
fi

if [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    echo "ANDROID_NDK_ROOT must point to an Android NDK installation" >&2
    exit 1
fi

"$ADB" get-state >/dev/null

echo "Using ANDROID_HOME=$ANDROID_HOME" >&2
echo "Using ANDROID_NDK_ROOT=$ANDROID_NDK_ROOT" >&2

cargo apk build -p inertia-scroll-probe --target "$TARGET" --lib

APK="$(find target -path '*/apk/*.apk' -type f | sort | tail -1)"
if [ -z "$APK" ]; then
    echo "No APK found under target/*/apk after cargo apk build" >&2
    exit 1
fi

"$ADB" install -r "$APK" >/dev/null
"$ADB" shell am force-stop "$PACKAGE" >/dev/null
"$ADB" logcat -c
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY" >/dev/null

echo "Collecting inertia trace from logcat for 5 seconds..." >&2
sleep 5
"$ADB" logcat -d | grep -E 'inertia-scroll-probe|source,gesture|slint,' || true
