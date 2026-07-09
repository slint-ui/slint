#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euo pipefail

PACKAGE="${INERTIA_PROBE_PACKAGE:-dev.slint.examples.inertiascrollprobe}"
TARGET="${ANDROID_TARGET:-aarch64-linux-android}"
ADB="${ADB:-adb}"

command -v cargo-apk >/dev/null || {
    echo "cargo-apk is required. Install it with: cargo install cargo-apk" >&2
    exit 1
}

command -v "$ADB" >/dev/null || {
    echo "adb is required. Put Android platform-tools on PATH or set ADB=/path/to/adb" >&2
    exit 1
}

if [ -z "${ANDROID_HOME:-}" ]; then
    echo "ANDROID_HOME must point to an Android SDK installation" >&2
    exit 1
fi

"$ADB" get-state >/dev/null

cargo apk build -p inertia-scroll-probe --target "$TARGET" --lib

APK="$(find target -path '*/apk/*.apk' -type f | sort | tail -1)"
if [ -z "$APK" ]; then
    echo "No APK found under target/*/apk after cargo apk build" >&2
    exit 1
fi

"$ADB" install -r "$APK" >/dev/null
"$ADB" logcat -c
"$ADB" shell monkey -p "$PACKAGE" 1 >/dev/null

echo "Collecting inertia trace from logcat for 5 seconds..." >&2
sleep 5
"$ADB" logcat -d | grep -E 'inertia-scroll-probe|source,gesture|slint,' || true
