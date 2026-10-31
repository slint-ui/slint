#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Builds slint-dotnet as a dynamic framework for iOS devices and the simulator on Apple Silicon,
# and combines them into target/ios/slint_dotnet.xcframework, which Slint.targets embeds into iOS apps.
# Usage: build_dotnet_ios.sh [cargo profile, release by default]

set -eu

profile=${1:-release}
cd "$(dirname "$0")/.."
export IPHONEOS_DEPLOYMENT_TARGET=15.0
# The features that iOS has: winit with Skia on Metal, without linuxkms and live preview.
features=backend-winit,renderer-skia
dir=$profile
[ "$profile" = dev ] && dir=debug

frameworks=()
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    cargo rustc --profile "$profile" -p slint-dotnet --target "$target" \
        --no-default-features --features "$features" --crate-type cdylib

    framework=target/ios/$target/slint_dotnet.framework
    rm -rf "$framework"
    mkdir -p "$framework"
    cp "target/$target/$dir/libslint_dotnet.dylib" "$framework/slint_dotnet"
    install_name_tool -id @rpath/slint_dotnet.framework/slint_dotnet "$framework/slint_dotnet"
    # Debug information would triple the size of the packages.
    [ "$profile" = release ] && strip -S -x "$framework/slint_dotnet"
    platform=iPhoneOS
    [ "$target" = aarch64-apple-ios-sim ] && platform=iPhoneSimulator
    cat > "$framework/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>slint_dotnet</string>
  <key>CFBundleIdentifier</key>
  <string>dev.slint.dotnet</string>
  <key>CFBundleName</key>
  <string>slint_dotnet</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleSupportedPlatforms</key>
  <array>
    <string>$platform</string>
  </array>
  <key>MinimumOSVersion</key>
  <string>$IPHONEOS_DEPLOYMENT_TARGET</string>
</dict>
</plist>
EOF
    frameworks+=(-framework "$framework")
done

rm -rf target/ios/slint_dotnet.xcframework
xcodebuild -create-xcframework "${frameworks[@]}" -output target/ios/slint_dotnet.xcframework
