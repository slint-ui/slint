#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -eu

if [ "$#" -ne 5 ]; then
    echo "Usage: $0 OUTPUT SLINT_VERSION PROTOCOL ANDROID_APK|- IOS_SIMULATOR_ZIP|-" >&2
    exit 2
fi

output=$1
slint_version=$2
protocol=$3
android_apk=$4
ios_simulator_zip=$5

android_file_name=
android_sha256=
if [ "$android_apk" != "-" ]; then
    android_file_name=$(basename "$android_apk")
    if command -v sha256sum >/dev/null 2>&1; then
        android_sha256=$(sha256sum "$android_apk" | cut -d ' ' -f 1)
    else
        android_sha256=$(shasum -a 256 "$android_apk" | cut -d ' ' -f 1)
    fi
fi

ios_file_name=
ios_sha256=
if [ "$ios_simulator_zip" != "-" ]; then
    ios_file_name=$(basename "$ios_simulator_zip")
    if command -v sha256sum >/dev/null 2>&1; then
        ios_sha256=$(sha256sum "$ios_simulator_zip" | cut -d ' ' -f 1)
    else
        ios_sha256=$(shasum -a 256 "$ios_simulator_zip" | cut -d ' ' -f 1)
    fi
fi

if [ -z "$android_file_name" ] && [ -z "$ios_file_name" ]; then
    echo "At least one viewer artifact must be provided" >&2
    exit 2
fi

jq -n \
    --arg slint_version "$slint_version" \
    --arg protocol "$protocol" \
    --arg android_file_name "$android_file_name" \
    --arg android_sha256 "$android_sha256" \
    --arg ios_file_name "$ios_file_name" \
    --arg ios_sha256 "$ios_sha256" \
    '{
        schema_version: 1,
        release_tag: "local",
        slint_version: $slint_version,
        protocol: $protocol,
        artifacts: [
            (if $android_file_name == "" then empty else {
                kind: "android-apk",
                file_name: $android_file_name,
                sha256: $android_sha256,
                bundle_id: "dev.slint.viewer",
                architectures: ["arm64-v8a", "armeabi-v7a", "x86_64"]
            } end),
            (if $ios_file_name == "" then empty else {
                kind: "ios-simulator-app",
                file_name: $ios_file_name,
                sha256: $ios_sha256,
                bundle_id: "dev.slint.slint-viewer",
                architectures: ["arm64", "x86_64"]
            } end)
        ]
    }' > "$output"
