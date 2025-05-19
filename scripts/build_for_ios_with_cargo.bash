#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

set -euvx

# Fix up PATH to work around https://github.com/rust-lang/rust/issues/80817 and add cargo.
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH:$HOME/.cargo/bin"

# based on https://github.com/mozilla/glean/blob/main/build-scripts/xc-universal-binary.sh

if [[ "$CONFIGURATION" != "Debug" ]]; then
    CARGO_PROFILE=release
    MAYBE_RELEASE=--release
else
    CARGO_PROFILE=debug
    MAYBE_RELEASE=
fi

# Make Cargo output cache files in Xcode's directories
export CARGO_TARGET_DIR="$DERIVED_FILE_DIR/cargo"

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
  IS_SIMULATOR=1
fi

executables=()
for arch in $ARCHS; do
    case "$arch" in
        arm64)
            if [ $IS_SIMULATOR -eq 0 ]; then
              CARGO_TARGET=aarch64-apple-ios
            else
              CARGO_TARGET=aarch64-apple-ios-sim
            fi
            ;;
        x86_64)
            export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
            CARGO_TARGET=x86_64-apple-ios
            ;;
    esac

    cargo build $MAYBE_RELEASE --target $CARGO_TARGET --bin $1

    executables+=("$DERIVED_FILE_DIR/cargo/$CARGO_TARGET/$CARGO_PROFILE/$1")
done

# Combine executables, and place them at the output path excepted by Xcode
lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" "${executables[@]}"

# Force code signing every run for device builds (non-simulator)
if [ $IS_SIMULATOR -eq 0 ]; then
    codesign --force --sign "${EXPANDED_CODE_SIGN_IDENTITY}" \
             --entitlements "${TARGET_TEMP_DIR}/${PRODUCT_NAME}.app.xcent" \
             "${TARGET_BUILD_DIR}/${EXECUTABLE_PATH}"
fi
