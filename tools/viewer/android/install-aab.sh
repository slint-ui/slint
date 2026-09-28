#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore bundletool BUNDLETOOL apks APKS
#
# Build APKs from the slint-viewer AAB with bundletool and install them on a
# connected device. Hand-test the same bundle Play would receive without
# going through a Play track.
#
# Usage: install-aab.sh [PATH_TO_AAB]
#
# Required:
#   bundletool on PATH (or $BUNDLETOOL pointing at the jar)
#   adb on PATH; device authorized for USB debugging
#   ANDROID_KEYSTORE_PATH / ANDROID_KEYSTORE_PASSWORD / ANDROID_KEYSTORE_ALIAS
#   for signing — Android refuses unsigned APKs.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
AAB="${1:-$PROJECT_DIR/app/build/outputs/bundle/release/slint-viewer.aab}"
[ -f "$AAB" ] || { echo "AAB not found at $AAB; run build-aab.sh first" >&2; exit 1; }

[ -n "${ANDROID_KEYSTORE_PATH:-}" ] || { echo "set ANDROID_KEYSTORE_PATH for signing" >&2; exit 1; }
[ -n "${ANDROID_KEYSTORE_PASSWORD:-}" ] || { echo "set ANDROID_KEYSTORE_PASSWORD" >&2; exit 1; }
[ -n "${ANDROID_KEYSTORE_ALIAS:-}" ] || { echo "set ANDROID_KEYSTORE_ALIAS" >&2; exit 1; }

# bundletool ships as a jar; honor a BUNDLETOOL override for it.
if [ -n "${BUNDLETOOL:-}" ] && [[ "$BUNDLETOOL" == *.jar ]]; then
    bt() { java -jar "$BUNDLETOOL" "$@"; }
elif command -v bundletool >/dev/null; then
    bt() { bundletool "$@"; }
else
    echo "install bundletool, or point \$BUNDLETOOL at the jar" >&2
    exit 1
fi

OUT_APKS="${AAB%.aab}.apks"
rm -f "$OUT_APKS"

# Cache the adb state so both branches agree, and separate "no device" from
# "unauthorized" so the user sees the actual fix.
ADB_STATE=$(adb get-state 2>&1 || true)
case "$ADB_STATE" in
    device)
        MODE_FLAG=(--connected-device)
        ;;
    *unauthorized*)
        echo "device is unauthorized; accept the USB debugging prompt on the phone and retry" >&2
        exit 1
        ;;
    *)
        echo "no device on adb (state: $ADB_STATE); building a universal APK instead" >&2
        MODE_FLAG=(--mode=universal)
        ;;
esac

bt build-apks \
    --bundle="$AAB" \
    --output="$OUT_APKS" \
    "${MODE_FLAG[@]}" \
    --ks="$ANDROID_KEYSTORE_PATH" \
    --ks-pass="pass:$ANDROID_KEYSTORE_PASSWORD" \
    --ks-key-alias="$ANDROID_KEYSTORE_ALIAS"

if [ "$ADB_STATE" = "device" ]; then
    bt install-apks --apks="$OUT_APKS"
    echo "Installed slint-viewer from $OUT_APKS"
else
    UNIVERSAL_APK="${AAB%.aab}-universal.apk"
    unzip -p "$OUT_APKS" universal.apk > "$UNIVERSAL_APK"
    echo "Universal APK extracted to $UNIVERSAL_APK; install with: adb install $UNIVERSAL_APK"
fi
