# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
# cspell:ignore plutil
set -euo pipefail

VERSION="2.9.3"
EXPECTED_SHA256="74a07da821f92b79310009954c0e15f350173374a3abe39095b4fc5096916be6"

cd "$(git rev-parse --show-toplevel)"

# The framework reports its own version, so a checkout that still has an older
# one from before a VERSION bump re-downloads instead of keeping it forever.
# Read it through the Resources symlink to stay off the versioned directory
# name, and don't leave a stamp file behind: the framework gets copied into the
# app bundle and signed as it is.
installed_version() {
    plutil -extract CFBundleShortVersionString raw Sparkle.framework/Resources/Info.plist 2>/dev/null
}

if [ -d "Sparkle.framework" ] && [ -x "sparkle-bin/sign_update" ] &&
    [ -x "sparkle-bin/generate_keys" ] && [ "$(installed_version)" = "$VERSION" ]; then
    echo "Sparkle ${VERSION} and its tools already exist"
    exit 0
fi

TEMP_DIR=$(mktemp -d)

echo "Downloading Sparkle ${VERSION}..."
# --fail so an error page doesn't get written out as if it were the archive, and
# retries because this runs in CI, where a hiccup shouldn't fail the build.
curl --fail --location --retry 3 --retry-all-errors -o "$TEMP_DIR/sparkle.tar.xz" \
    "https://github.com/sparkle-project/Sparkle/releases/download/${VERSION}/Sparkle-${VERSION}.tar.xz"

echo "Verifying checksum..."
echo "${EXPECTED_SHA256}  $TEMP_DIR/sparkle.tar.xz" | shasum -a 256 -c -

echo "Extracting Sparkle.framework..."
tar -xf "$TEMP_DIR/sparkle.tar.xz" -C "$TEMP_DIR"

rm -rf Sparkle.framework sparkle-bin
ditto "$TEMP_DIR/Sparkle.framework" Sparkle.framework

# Also copy the bin tools (generate_keys, sign_update)
if [ -d "$TEMP_DIR/bin" ]; then
    echo "Copying Sparkle bin tools..."
    ditto "$TEMP_DIR/bin" sparkle-bin
    chmod +x ./sparkle-bin/*
fi

echo "Done!"
echo ""
echo "To generate EdDSA keys for signing updates:"
echo "  ./sparkle-bin/generate_keys --account slint-visual-editor"
echo "  ./sparkle-bin/generate_keys --account slint-visual-editor -p"
echo "  ./sparkle-bin/generate_keys --account slint-visual-editor -x /tmp/slint-visual-editor-sparkle-private-key"
