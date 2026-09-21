#!/bin/zsh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -e

serial=${ADB_SERIAL:?Set ADB_SERIAL to the attached device serial}
package=dev.slint.androidscrollcomparison
activity="$package/android.app.NativeActivity"
out=${OUTPUT_DIR:-/tmp/android-scroll-traces}
mkdir -p "$out"

for duration in 1000 500 250 125; do
  for side in native slint; do
    if [[ "$side" == native ]]; then
      x=270
    else
      x=810
    fi
    adb -s "$serial" shell am force-stop "$package"
    adb -s "$serial" shell am start -n "$activity" --ez native_control false >/dev/null
    sleep 1
    adb -s "$serial" logcat -c
    adb -s "$serial" shell input swipe "$x" 1800 "$x" 700 "$duration"
    sleep 4
    adb -s "$serial" logcat -d -v epoch \
      -s ScrollCompare:I android_native_slint_scroll:I '*:S' \
      > "$out/$side-d$duration.log"
  done
done
