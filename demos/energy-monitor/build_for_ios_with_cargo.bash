#!/usr/bin/env bash

set -euvx

# based on https://github.com/mozilla/glean/blob/main/build-scripts/xc-universal-binary.sh

# Xcode places `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin`
# at the front of the path, with makes the build fail with `ld: library 'System' not found`, upstream issue:
# <https://github.com/rust-lang/rust/issues/80817>.
#
# Work around it by resetting the path, so that we use the system `cc`.
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

PATH=$PATH:$HOME/.cargo/bin

PROFILE=debug
RELFLAG=
if [[ "$CONFIGURATION" != "Debug" ]]; then
    PROFILE=release
    RELFLAG=--release
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
        x86_64)
          export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
          TARGET=x86_64-apple-ios
          ;;
        arm64)
          if [ $IS_SIMULATOR -eq 0 ]; then
            TARGET=aarch64-apple-ios
          else
            TARGET=aarch64-apple-ios-sim
          fi
  esac

  cargo build $RELFLAG --target $TARGET --bin energy-monitor --no-default-features --features ios

  executables+=("$DERIVED_FILE_DIR/cargo/$TARGET/$PROFILE/energy-monitor")
done

# Combine executables, and place them at the output path excepted by Xcode
lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" "${executables[@]}"
