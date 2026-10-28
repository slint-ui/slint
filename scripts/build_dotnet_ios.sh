#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Builds slint-dotnet as a static library for iOS devices and the simulator on Apple Silicon,
# and combines them into target/ios/slint_dotnet.xcframework, which Slint.targets links into iOS apps.
# Usage: build_dotnet_ios.sh [cargo profile, release by default]

set -eu

profile=${1:-release}
cd "$(dirname "$0")/.."
# The features that iOS has: winit with Skia on Metal, without linuxkms and live preview.
features=backend-winit,renderer-skia
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo rustc --profile "$profile" -p slint-dotnet --target "$target" \
        --no-default-features --features "$features" --crate-type staticlib
done

dir=$profile
[ "$profile" = dev ] && dir=debug
rm -rf target/ios/slint_dotnet.xcframework
xcodebuild -create-xcframework \
    -library "target/aarch64-apple-ios/$dir/libslint_dotnet.a" \
    -library "target/aarch64-apple-ios-sim/$dir/libslint_dotnet.a" \
    -output target/ios/slint_dotnet.xcframework
