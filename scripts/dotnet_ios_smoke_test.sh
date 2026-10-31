#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Installs an app bundle of a Slint .NET app in an iPhone simulator, starts it,
# and fails if it doesn't keep running, or logs an unhandled exception, a panic, or a fatal signal.
# Usage: dotnet_ios_smoke_test.sh <app bundle> <bundle id> [seconds] [screenshot.png]

set -eu

app=$1
bundle_id=$2
seconds=${3:-15}
screenshot=${4:-}
executable=$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$app/Info.plist")

device=$(xcrun simctl list devices available -j |
    python3 -c 'import json, sys; print(next(d["udid"] for r in json.load(sys.stdin)["devices"].values() for d in r if d["name"].startswith("iPhone")))')
xcrun simctl boot "$device" 2> /dev/null || true
xcrun simctl bootstatus "$device" -b > /dev/null

xcrun simctl terminate "$device" "$bundle_id" 2> /dev/null || true
xcrun simctl install "$device" "$app"
log=$(mktemp)
xcrun simctl launch --console-pty "$device" "$bundle_id" > "$log" 2>&1 &
sleep "$seconds"

# A simulator app is a process of the host, at the path of its bundle.
running=$(pgrep -f "$executable.app/$executable" || true)
errors=$(grep -E "Unhandled managed exception|panicked|Got a SIG" "$log" || true)
[ -n "$screenshot" ] && xcrun simctl io "$device" screenshot "$screenshot" > /dev/null 2>&1
if [ -z "$running" ] || [ -n "$errors" ]; then
    [ -z "$running" ] && echo "$bundle_id isn't running after $seconds seconds"
    [ -n "$errors" ] && printf '%s logged errors:\n%s\n' "$bundle_id" "$errors"
    echo "The output of $bundle_id:"
    tail -n 60 "$log"
    exit 1
fi
echo "$bundle_id is running as process $running"
xcrun simctl terminate "$device" "$bundle_id"
