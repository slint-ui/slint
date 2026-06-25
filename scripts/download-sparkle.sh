#!/bin/bash
set -euo pipefail

VERSION="2.9.3"
EXPECTED_SHA256="74a07da821f92b79310009954c0e15f350173374a3abe39095b4fc5096916be6"

cd "$(git rev-parse --show-toplevel)"

if [ -d "Sparkle.framework" ]; then
    echo "Sparkle.framework already exists"
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

cp -R "$TEMP_DIR/Sparkle.framework" .

# Also copy the bin tools (generate_keys, sign_update)
if [ -d "$TEMP_DIR/bin" ]; then
    echo "Linking Sparkle bin tools..."
    ln -s "$TEMP_DIR/bin" ./sparkle-bin
    chmod +x ./sparkle-bin/*
fi

echo "Done!"
echo ""
echo "To generate EdDSA keys for signing updates:"
echo "  ./sparkle-bin/generate_keys"
