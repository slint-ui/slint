#!/bin/zsh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT
# cspell:ignore androidscrollcomparison logcat

set -e

serial=${ADB_SERIAL:?Set ADB_SERIAL to the attached device serial}
package=dev.slint.androidscrollcomparison
activity="$package/android.app.NativeActivity"
out=${OUTPUT_DIR:-/tmp/android-scroll-traces}
mkdir -p "$out"

size=$(adb -s "$serial" shell wm size | sed -n 's/.*Physical size: \([0-9]*x[0-9]*\).*/\1/p')
width=${size%x*}
height=${size#*x}
density=$(adb -s "$serial" shell wm density | sed -n 's/.*Physical density: \([0-9]*\).*/\1/p')
x=$((width / 2))

run_gesture() {
  local name=$1
  local start_y=$2
  local end_y=$3
  local duration=$4
  adb -s "$serial" shell am force-stop "$package"
  adb -s "$serial" shell am start -n "$activity" --ez native_control false >/dev/null
  sleep 1
  adb -s "$serial" logcat -c
  adb -s "$serial" shell input swipe "$x" "$start_y" "$x" "$end_y" "$duration"
  sleep 6
  adb -s "$serial" logcat -d -v epoch \
    -s ScrollCompare:I android_native_slint_scroll:I '*:S' \
    > "$out/$name.log"
}

for duration in 1000 500 250 125; do
  run_gesture "shared-d$duration" $((height * 77 / 100)) $((height * 30 / 100)) "$duration"
done

short_distance=$((density * 120 / 160))
short_start=$((height * 72 / 100))
short_end=$((short_start - short_distance))
for trial in 1 2 3; do
  run_gesture "short-hard-d120-t20ms-trial-$trial" \
    "$short_start" "$short_end" 20
done
