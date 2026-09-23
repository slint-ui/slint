#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euo pipefail

artifact_dir=${SCROLL_ARTIFACT_DIR:?Set SCROLL_ARTIFACT_DIR to the artifact directory}
package=dev.slint.androidscrollcomparison
activity="$package/android.app.NativeActivity"
apk=${SCROLL_APK:?Set SCROLL_APK to the comparison APK}
mkdir -p "$artifact_dir"

adb install -r "$apk"
adb logcat -c
adb shell am force-stop "$package" || true
adb shell am start -n "$activity"
sleep 3

size=$(adb shell wm size | sed -n 's/.*Physical size: \([0-9]*x[0-9]*\).*/\1/p')
width=${size%x*}
height=${size#*x}
x=$((width / 2))
adb shell input swipe "$x" "$((height * 75 / 100))" "$x" "$((height * 30 / 100))" 500
sleep 2

adb logcat -d -v tag > "$artifact_dir/logcat.txt"
adb exec-out screencap -p > "$artifact_dir/screenshot.png"
python3 "$(dirname "$0")/check-smoke.py" "$artifact_dir/logcat.txt" \
  > "$artifact_dir/offset-check.txt"
cat "$artifact_dir/offset-check.txt"
