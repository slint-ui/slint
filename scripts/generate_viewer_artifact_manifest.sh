#!/bin/sh
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

set -eu

if [ "$#" -ne 5 ]; then
    echo "Usage: $0 OUTPUT RELEASE_TAG SLINT_VERSION PROTOCOL ANDROID_APK" >&2
    exit 2
fi

output=$1
release_tag=$2
slint_version=$3
protocol=$4
android_apk=$5

if command -v sha256sum >/dev/null 2>&1; then
    android_sha256=$(sha256sum "$android_apk" | cut -d ' ' -f 1)
else
    android_sha256=$(shasum -a 256 "$android_apk" | cut -d ' ' -f 1)
fi

jq -n \
    --arg release_tag "$release_tag" \
    --arg slint_version "$slint_version" \
    --arg protocol "$protocol" \
    --arg android_file_name "$(basename "$android_apk")" \
    --arg android_sha256 "$android_sha256" \
    '{
        schema_version: 1,
        release_tag: $release_tag,
        slint_version: $slint_version,
        protocol: $protocol,
        artifacts: [
            {
                kind: "android-apk",
                file_name: $android_file_name,
                sha256: $android_sha256,
                bundle_id: "dev.slint.viewer",
                architectures: ["arm64-v8a", "armeabi-v7a", "x86_64"]
            }
        ]
    }' > "$output"
