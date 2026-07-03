#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Publish all public crates to crates.io in one `cargo publish` invocation,
# which uploads them in dependency order.
# Extra arguments (e.g. --dry-run, --no-verify) are forwarded to cargo publish.
#
# The helper crates (vtable, const-field-offset) have their own version
# numbers and are published manually when needed.
#
# The release CI authenticates via crates.io trusted publishing, which only
# works for existing crates. To add a crate to this list, publish a dummy
# 0.0.0 version manually, then configure trusted publishing in the crate's
# settings on crates.io (repository slint-ui/slint, workflow nightly_snapshot.yaml).

exec cargo publish "$@" \
    -p i-slint-common \
    -p i-slint-core-macros \
    -p i-slint-compiler \
    -p i-slint-core \
    -p slint-macros \
    -p i-slint-renderer-skia --features i-slint-renderer-skia/x11 \
    -p i-slint-renderer-femtovg \
    -p i-slint-renderer-software \
    -p i-slint-backend-winit --features i-slint-backend-winit/x11,i-slint-backend-winit/renderer-femtovg \
    -p slint-build \
    -p i-slint-backend-qt \
    -p i-slint-backend-linuxkms \
    -p i-slint-backend-android-activity --features i-slint-backend-android-activity/native-activity \
    -p i-slint-backend-testing \
    -p i-slint-backend-selector --features i-slint-backend-selector/backend-winit-x11,i-slint-backend-selector/renderer-femtovg \
    -p slint-interpreter \
    -p i-slint-live-preview \
    -p slint \
    -p slint-lsp \
    -p slint-viewer \
    -p slint-tr-extractor
