#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cSpell:ignore tlsv1
# Xcode Cloud post-clone hook for the slint-viewer iOS app.
#
# Steps:
#   1. Install xcodegen (project generator) and librsvg (rsvg-convert, used by
#      scripts/render_ios_app_icon.bash to render the app icon at build time).
#   2. Install rustup with the stable toolchain and the iOS Rust targets.
#   3. Run xcodegen to materialize tools/viewer/Slint Viewer.xcodeproj.

set -euo pipefail

echo "--- Installing xcodegen + librsvg via Homebrew"
brew install xcodegen librsvg

echo "--- Installing rustup + stable toolchain"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
export PATH="$HOME/.cargo/bin:$PATH"

echo "--- Adding iOS Rust targets"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# Xcode Cloud places us in tools/viewer/ci_scripts on entry; the project lives
# one level up. CI_PRIMARY_REPOSITORY_PATH points to the repo root.
cd "$CI_PRIMARY_REPOSITORY_PATH/tools/viewer"

echo "--- Generating Xcode project"
xcodegen generate --spec ios-project.yml
