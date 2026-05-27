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
#   3. Compute a monotonic build number from git history and rewrite the
#      xcodegen spec so the generated Info.plist carries the right
#      CFBundleVersion (App Store Connect rejects re-uploads with stale ones).
#   4. Run xcodegen to materialise tools/viewer/Slint Viewer.xcodeproj.
#
# The TestFlight "What to Test" notes are kept manually in
# tools/viewer/TestFlight/WhatToTest.en-US.txt and committed in advance of a
# release; this script does not touch them.

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

echo "--- Computing build number from git"
# Xcode Cloud clones the repo shallow (depth 1), which would make
# `git rev-list --count HEAD` silently return 1 on every run and trip App
# Store Connect's "build number must increase" rule on the second upload.
# Unshallow first so the commit count is accurate and matches the convention
# used by the upstream GHA build_ios job in slint_tool_binary.yaml.
git -C "$CI_PRIMARY_REPOSITORY_PATH" fetch --unshallow --quiet || \
    git -C "$CI_PRIMARY_REPOSITORY_PATH" fetch --quiet
CURRENT_PROJECT_VERSION=$(git -C "$CI_PRIMARY_REPOSITORY_PATH" rev-list --count HEAD)
echo "Build number: $CURRENT_PROJECT_VERSION"

# Patch the spec so xcodegen bakes the build number into Info.plist. Just
# exporting CURRENT_PROJECT_VERSION as an env var would not reach xcodebuild.
sed -i.bak \
    -E "s/^  CURRENT_PROJECT_VERSION: .*/  CURRENT_PROJECT_VERSION: \"$CURRENT_PROJECT_VERSION\"/" \
    ios-project.yml
rm ios-project.yml.bak

echo "--- Generating Xcode project"
xcodegen generate --spec ios-project.yml
