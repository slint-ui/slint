#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Build Node.js as a shared library (libnode).
#
# Usage:
#   ./build-libnode.sh [--version 22.22.2] [--prefix /opt/libnode]
#
# The output directory can then be passed to CMake:
#   cmake -DNODE_DIR=/opt/libnode ..
#
# Requirements: Python 3, GCC/Clang, make

set -euo pipefail

NODE_VERSION="22.22.2"
PREFIX="$(pwd)/libnode-install"
JOBS="${CMAKE_BUILD_PARALLEL_LEVEL:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) NODE_VERSION="$2"; shift 2 ;;
        --prefix)  PREFIX="$2"; shift 2 ;;
        --jobs|-j) JOBS="$2"; shift 2 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

echo "Building Node.js v${NODE_VERSION} as shared library"
echo "  prefix: ${PREFIX}"
echo "  jobs:   ${JOBS}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "${WORKDIR}"' EXIT

TARBALL="node-v${NODE_VERSION}.tar.gz"
URL="https://nodejs.org/dist/v${NODE_VERSION}/${TARBALL}"

echo "Downloading ${URL} ..."
curl -fsSL -o "${WORKDIR}/${TARBALL}" "${URL}"

echo "Extracting ..."
tar xzf "${WORKDIR}/${TARBALL}" -C "${WORKDIR}"

cd "${WORKDIR}/node-v${NODE_VERSION}"

echo "Configuring (--shared --prefix=${PREFIX}) ..."
./configure --shared --prefix="${PREFIX}"

echo "Building (${JOBS} parallel jobs) ..."
make "-j${JOBS}"

echo "Installing to ${PREFIX} ..."
make install

echo ""
echo "Done.  Use with CMake:"
echo "  cmake -DNODE_DIR=${PREFIX} .."
