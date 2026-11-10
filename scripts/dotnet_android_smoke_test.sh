#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore monodroid

# Installs an APK of a Slint .NET app on the device that adb uses, starts it,
# and fails if it doesn't keep running, or logs a crash or an error of SlintActivity.
# Usage: dotnet_android_smoke_test.sh <apk> <application id> [seconds]

set -euo pipefail

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

# Runs adb, and again after reconnecting when adb itself fails, such as with "device offline",
# so that a lost connection doesn't look like an app that ended.
query() {
    for _ in 1 2 3; do
        if output=$(adb "$@"); then
            printf '%s\n' "$output"
            return
        fi
        reconnect
    done
    echo "adb keeps losing the device" >&2
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
log=$(query logcat -d)
errors=$(grep -E " E Slint +:|RustStdoutStderr: .*panicked|Fatal signal.*\\(${app: -15}\\)" <<< "$log" || true)
# pidof fails when there's no such process, which mustn't count as a failure of adb.
pid=$(query shell "pidof $app || true" | tr -d '\r')
if [ -z "$pid" ] || [ -n "$errors" ]; then
    [ -z "$pid" ] && echo "$app isn't running after $seconds seconds"
    [ -n "$errors" ] && printf '%s logged errors:\n%s\n' "$app" "$errors"
    echo "Log lines about $app, such as why its process ended:"
    grep -E "AndroidRuntime|monodroid|DOTNET|$app|${app: -15}" <<< "$log" | tail -n 60 || true
    exit 1
fi
echo "$app is running as process $pid"
adb shell am force-stop "$app"
