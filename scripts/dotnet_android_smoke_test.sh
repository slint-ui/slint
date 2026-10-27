#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Installs an APK of a Slint .NET app on the device that adb uses, starts it,
# and fails if it doesn't keep running, or logs a crash or an error of SlintActivity.
# Usage: dotnet_android_smoke_test.sh <apk> <application id> [seconds]

set -eu

apk=$1
app=$2
seconds=${3:-15}

# Waits up to 2 minutes for adb to answer, since an emulator under load drops the connection for a while.
reconnect() {
    for _ in $(seq 1 24); do
        adb wait-for-device shell true 2> /dev/null && return
        sleep 5
    done
    echo "adb doesn't reach the device"
    exit 1
}

adb install -r "$apk"
adb logcat -c
activity=$(adb shell cmd package resolve-activity --brief "$app" | tail -n 1 | tr -d '\r')
adb shell am start -W -n "$activity" > /dev/null
sleep "$seconds"
reconnect

# SlintActivity logs an exception of SlintMain under the tag Slint, and the app then ends.
# Logcat names a crashed process by the last 15 characters of its name, and other processes crash on emulators too.
errors=$(adb logcat -d | grep -E " E Slint +:|RustStdoutStderr|Fatal signal.*\\(${app: -15}\\)" || true)
pid=$(adb shell pidof "$app" | tr -d '\r' || true)
if [ -z "$pid" ] || [ -n "$errors" ]; then
    [ -z "$pid" ] && echo "$app isn't running after $seconds seconds"
    [ -n "$errors" ] && printf '%s logged errors:\n%s\n' "$app" "$errors"
    exit 1
fi
echo "$app is running as process $pid"
adb shell am force-stop "$app"
