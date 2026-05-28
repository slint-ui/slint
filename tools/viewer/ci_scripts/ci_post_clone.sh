#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Xcode Cloud post-clone hook for the slint-viewer iOS app.
#
# Runs after the repository is cloned and before Xcode Cloud validates the
# project path + scheme, so we have to do *all* project generation here (the
# ci_pre_xcodebuild.sh hook runs too late).
#
# Steps:
#   1. Install xcodegen (project generator) and librsvg (rsvg-convert, used by
#      scripts/render_ios_app_icon.bash to render the app icon at build time).
#   2. Install rustup with the stable toolchain and the iOS Rust targets.
#   3. Read the workspace version from Cargo.toml and rewrite the xcodegen spec
#      so Info.plist carries the right CFBundleShortVersionString. Build numbers
#      (CFBundleVersion) are assigned by Xcode Cloud, not from git history.
#   4. Run xcodegen to materialise tools/viewer/Slint Viewer.xcodeproj.

set -euo pipefail

echo "--- Installing xcodegen + librsvg via Homebrew"
brew install xcodegen librsvg

echo "--- Installing rustup + stable toolchain"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
export PATH="$HOME/.cargo/bin:$PATH"

echo "--- Adding iOS Rust targets"
# aarch64-apple-ios for device archives (what Xcode Cloud needs for TestFlight);
# the simulator triple is included so simulator-targeted workflows also work.
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# Xcode Cloud places us in tools/viewer/ci_scripts on entry; the project lives
# one level up. CI_PRIMARY_REPOSITORY_PATH points to the repo root.
cd "$CI_PRIMARY_REPOSITORY_PATH/tools/viewer"

echo "--- Computing marketing version"
MARKETING_VERSION=$(
    sed -n '/\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' \
        "$CI_PRIMARY_REPOSITORY_PATH/Cargo.toml"
)
echo "Marketing version: $MARKETING_VERSION"

# Patch the spec so xcodegen bakes the marketing version into Info.plist.
sed -i.bak \
    -E "s/^  MARKETING_VERSION: .*/  MARKETING_VERSION: \"$MARKETING_VERSION\"/" \
    ios-project.yml
rm ios-project.yml.bak

echo "--- Generating Xcode project"
xcodegen generate --spec ios-project.yml
