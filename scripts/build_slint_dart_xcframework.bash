#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Bundle `libslint_dart` for every Apple platform into SlintDart.xcframework.
#
# The slices are dynamic frameworks, the same shape the build hook already
# bundles on macOS, so `package:slint` loads them the way it loads the library
# everywhere else. Embed the xcframework in the Xcode target ("Embed & Sign")
# and `DynamicLibrary.open` finds it at runtime.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
PROFILE="${SLINT_DART_PROFILE:-release}"
OUTPUT="${SLINT_DART_XCFRAMEWORK:-$TARGET_DIR/SlintDart.xcframework}"
STAGE="$TARGET_DIR/xcframework-slices"

# The framework's bundle name, which is also the name of the binary inside it
# and what `ffi.dart` looks for.
NAME=slint_dart
BUNDLE_ID=dev.slint.slintdart
VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-13.0}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

# Only the software renderer: on Apple platforms the Dart binding always draws
# through the embedded surface in `api/flutter/rust/embedded.rs`, never through
# a native window, so winit, Skia and FemtoVG would be dead weight. Override to
# build a different set.
FEATURES="${SLINT_DART_FEATURES:---no-default-features --features renderer-software}"

IOS_TARGETS=(aarch64-apple-ios)
IOS_SIMULATOR_TARGETS=(aarch64-apple-ios-sim x86_64-apple-ios)
MACOS_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

# Progress goes to stderr throughout: stdout of these helpers is the path that
# the command substitutions below capture.

write_plist() {
    local path="$1" platform="$2" min_os="$3"
    cat >"$path" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>en</string>
	<key>CFBundleExecutable</key><string>$NAME</string>
	<key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleName</key><string>$NAME</string>
	<key>CFBundlePackageType</key><string>FMWK</string>
	<key>CFBundleShortVersionString</key><string>$VERSION</string>
	<key>CFBundleVersion</key><string>$VERSION</string>
	<key>CFBundleSupportedPlatforms</key><array><string>$platform</string></array>
	<key>MinimumOSVersion</key><string>$min_os</string>
</dict>
</plist>
PLIST
}

# Build every architecture of one platform, merge them, and lay the result out
# as a framework. iOS uses the flat layout and macOS the versioned one; Apple
# rejects either in the other's place.
make_framework() {
    local slice="$1" layout="$2" platform="$3" min_os="$4"
    shift 4

    local dylibs=()
    for target in "$@"; do
        echo "==> building slint-dart for $target" >&2
        # shellcheck disable=SC2086 # FEATURES is a deliberate argument list.
        cargo build --profile "$PROFILE" --package slint-dart --target "$target" $FEATURES >&2
        dylibs+=("$TARGET_DIR/$target/$PROFILE/lib$NAME.dylib")
    done

    local framework="$STAGE/$slice/$NAME.framework"
    rm -rf "$framework"
    local binary plist install_name
    if [ "$layout" = versioned ]; then
        mkdir -p "$framework/Versions/A/Resources"
        binary="$framework/Versions/A/$NAME"
        plist="$framework/Versions/A/Resources/Info.plist"
        install_name="@rpath/$NAME.framework/Versions/A/$NAME"
        ln -s A "$framework/Versions/Current"
        ln -s "Versions/Current/$NAME" "$framework/$NAME"
        ln -s Versions/Current/Resources "$framework/Resources"
    else
        mkdir -p "$framework"
        binary="$framework/$NAME"
        plist="$framework/Info.plist"
        install_name="@rpath/$NAME.framework/$NAME"
    fi

    lipo -create -output "$binary" "${dylibs[@]}"
    # Cargo links the cdylib with an absolute install name under `target/`,
    # which no application can resolve. The loader has to find it inside
    # whichever bundle embeds the framework instead.
    install_name_tool -id "$install_name" "$binary"
    write_plist "$plist" "$platform" "$min_os"

    echo "$framework"
}

ios=$(make_framework ios flat iPhoneOS "$IPHONEOS_DEPLOYMENT_TARGET" "${IOS_TARGETS[@]}")
ios_simulator=$(make_framework ios-simulator flat iPhoneSimulator \
    "$IPHONEOS_DEPLOYMENT_TARGET" "${IOS_SIMULATOR_TARGETS[@]}")
macos=$(make_framework macos versioned MacOSX "$MACOSX_DEPLOYMENT_TARGET" \
    "${MACOS_TARGETS[@]}")

# xcodebuild refuses to overwrite an existing bundle.
rm -rf "$OUTPUT"

xcodebuild -create-xcframework \
    -framework "$ios" \
    -framework "$ios_simulator" \
    -framework "$macos" \
    -output "$OUTPUT"

echo
echo "$OUTPUT"
echo "Add it to the Xcode target's 'Frameworks, Libraries, and Embedded"
echo "Content' with 'Embed & Sign'."
