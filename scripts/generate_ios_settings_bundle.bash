#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Generate the iOS Settings.bundle that surfaces the third-party license
# attribution in the system Settings app. Run as a pre-build script by
# ios-project.yml; the bundle is then copied into the .app as a resource.

set -euo pipefail

# Fix up PATH to add cargo (Xcode's PATH may lack ~/.cargo/bin).
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH:$HOME/.cargo/bin"

# The attribution data spans all target platforms, so a single invocation
# suffices regardless of the architectures being built. cwd is the viewer
# crate (tools/viewer), so the analyzed manifest is the viewer's.
cargo xtask license --format ios-settings-bundle -o "$SRCROOT/Settings.bundle"
