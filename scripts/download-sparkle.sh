# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#!/bin/bash
set -euo pipefail

VERSION="2.9.3"
EXPECTED_SHA256="74a07da821f92b79310009954c0e15f350173374a3abe39095b4fc5096916be6"

cd "$(git rev-parse --show-toplevel)"

if [ -d "Sparkle.framework" ] && [ -x "sparkle-bin/sign_update" ] && [ -x "sparkle-bin/generate_keys" ]; then
    echo "Sparkle.framework and Sparkle tools already exist"
    exit 0
fi

TEMP_DIR=$(mktemp -d)

echo "Downloading Sparkle ${VERSION}..."
curl -L -o "$TEMP_DIR/sparkle.tar.xz" \
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
