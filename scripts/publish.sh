#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Since Cargo 1.90, Cargo can resolve the dependency order by itself
# We do not have to worry about the order here.
cargo publish --manifest-path Cargo.toml \
    --package i-slint-common \
    --package i-slint-core-macros \
    --package i-slint-compiler \
    --package i-slint-core \
    --package slint-macros \
    --package i-slint-renderer-skia \
    --package i-slint-renderer-femtovg \
    --package i-slint-renderer-software \
    --package i-slint-backend-winit \
    --package slint-build \
    --package i-slint-backend-qt \
    --package i-slint-backend-linuxkms \
    --package i-slint-backend-android-activity \
    --package i-slint-backend-testing \
    --package i-slint-backend-selector \
    --package slint-interpreter \
    --package i-slint-live-preview \
    --package slint \
    --package slint-lsp \
    --package slint-viewer \
    --package slint-tr-extractor \
    --features i-slint-renderer-skia/x11 \
    --features i-slint-backend-winit/x11 \
    --features i-slint-backend-winit/renderer-femtovg \
    --features i-slint-backend-android-activity/native-activity \
    --features i-slint-backend-selector/backend-winit-x11 \
    --features i-slint-backend-selector/renderer-femtovg
