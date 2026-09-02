#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore DSLINT
#
# Assert how the LinuxKMS backend features resolve to the libseat and libinput
# system dependencies.
#
# `backend-linuxkms` is the bare backend, and `compat-1-2` restores its pre-1.18
# meaning of libseat + libinput. That implication is carried by a weak feature
# reference on an optional dependency declared in the slint and slint-interpreter
# manifests; it deliberately does not go through i-slint-backend-selector, whose
# edge to the backend `backend-linuxkms-noseat` also activates. Getting that
# wrong compiles perfectly well and silently changes what an application links
# against, so it is checked here rather than left to review.
#
# Runs `cargo tree` and CMake on a project without languages only, no compilation,
# and needs no system libraries.

set -euo pipefail
cd "$(dirname "$0")/../../.."

failures=0

# has <package> <cargo-tree-args...> -- true if <package> is in the resolved graph.
# `cargo tree -i` exits 0 with "nothing to print" for a package that is absent on
# the host target, so match the output instead of the exit status.
has() {
    local pkg=$1; shift
    cargo tree "$@" -i "$pkg" 2>/dev/null | head -n1 | grep -q "^${pkg} v"
}

# expect <description> <want-libseat> <want-libinput> <cargo-tree-args...>
expect() {
    local desc=$1 want_seat=$2 want_input=$3; shift 3
    local seat=false input=false
    has libseat "$@" && seat=true
    # The `input` crate reaches the graph only through the backend's libinput
    # feature or through `unstable-libinput-09`, which forces that same feature.
    has input "$@" && input=true
    if [ "$seat" = "$want_seat" ] && [ "$input" = "$want_input" ]; then
        printf 'ok   %-54s libseat=%-5s libinput=%s\n' "$desc" "$seat" "$input"
    else
        printf 'FAIL %-54s libseat=%-5s libinput=%-5s (wanted libseat=%s libinput=%s)\n' \
            "$desc" "$seat" "$input" "$want_seat" "$want_input"
        failures=$((failures + 1))
    fi
}

echo "== slint: pre-1.18 spellings must keep resolving exactly as before =="
expect "default + backend-linuxkms" true true \
    -p slint --features backend-linuxkms
expect "default + backend-linuxkms-noseat" false true \
    -p slint --features backend-linuxkms-noseat
expect "compat-1-2 + backend-linuxkms" true true \
    -p slint --no-default-features --features compat-1-2,backend-linuxkms
expect "compat-1-2 + backend-linuxkms-noseat" false true \
    -p slint --no-default-features --features compat-1-2,backend-linuxkms-noseat
expect "compat-1-0 + backend-linuxkms-noseat" false true \
    -p slint --no-default-features --features compat-1-0,backend-linuxkms-noseat

echo "== slint: compat-1-18 makes backend-linuxkms the bare backend =="
expect "compat-1-18 + backend-linuxkms" false false \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms
expect "compat-1-18 + backend-linuxkms-libseat" true false \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms-libseat
expect "compat-1-18 + backend-linuxkms-libinput" false true \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms-libinput
expect "compat-1-18 + backend-linuxkms-noseat" false true \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms-noseat
expect "compat-1-18 + libseat + libinput" true true \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms-libseat,backend-linuxkms-libinput
expect "compat-1-18 + backend-linuxkms + unstable-libinput-09" false true \
    -p slint --no-default-features --features compat-1-18,backend-linuxkms,unstable-libinput-09

echo "== slint-interpreter: same implication as slint =="
expect "default + backend-linuxkms" true true \
    -p slint-interpreter --features backend-linuxkms
expect "default + backend-linuxkms-noseat" false true \
    -p slint-interpreter --features backend-linuxkms-noseat

# expect_cmake <description> <want-libseat> <want-libinput> <cmake -D options...>
# Configures api/cpp/cmake/SlintFeatures.cmake with the options in a fresh build
# directory and resolves the cargo features it selects for slint-cpp.
cmake_dir=$(mktemp -d)
trap 'rm -rf "$cmake_dir"' EXIT
cat > "$cmake_dir/CMakeLists.txt" <<EOF
cmake_minimum_required(VERSION 3.21)
project(check NONE)
include($PWD/api/cpp/cmake/SlintFeatures.cmake)
message(STATUS "features=\${features}")
EOF
configure() {
    cmake -S "$cmake_dir" -B "$cmake_dir/build" "$@" | sed -n 's/^-- features=//p' | tr ';' ','
}
expect_cmake() {
    local desc=$1 want_seat=$2 want_input=$3; shift 3
    local features
    rm -rf "$cmake_dir/build"
    features=$(configure "$@")
    expect "$desc" "$want_seat" "$want_input" -p slint-cpp --features "$features"
}

echo "== slint-cpp: each CMake option selects a capability, and they compose =="
expect "backend-linuxkms" false false \
    -p slint-cpp --features backend-linuxkms
expect "backend-linuxkms-libseat" true false \
    -p slint-cpp --features backend-linuxkms-libseat
expect "backend-linuxkms-libinput" false true \
    -p slint-cpp --features backend-linuxkms-libinput
expect "backend-linuxkms-libseat + libinput" true true \
    -p slint-cpp --features backend-linuxkms-libseat,backend-linuxkms-libinput

echo "== CMake: SLINT_FEATURE_BACKEND_LINUXKMS selects both, the capabilities compose =="
expect_cmake "no LinuxKMS option" false false
expect_cmake "BACKEND_LINUXKMS" true true \
    -DSLINT_FEATURE_BACKEND_LINUXKMS=ON
expect_cmake "BACKEND_LINUXKMS + LIBSEAT=OFF" false true \
    -DSLINT_FEATURE_BACKEND_LINUXKMS=ON -DSLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT=OFF
expect_cmake "BACKEND_LINUXKMS + LIBSEAT=OFF + LIBINPUT=OFF" false false \
    -DSLINT_FEATURE_BACKEND_LINUXKMS=ON -DSLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT=OFF -DSLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT=OFF
expect_cmake "BACKEND_LINUXKMS_LIBSEAT" true false \
    -DSLINT_FEATURE_BACKEND_LINUXKMS_LIBSEAT=ON
expect_cmake "BACKEND_LINUXKMS_LIBINPUT" false true \
    -DSLINT_FEATURE_BACKEND_LINUXKMS_LIBINPUT=ON
expect_cmake "BACKEND_LINUXKMS_NOSEAT (deprecated)" false true \
    -DSLINT_FEATURE_BACKEND_LINUXKMS_NOSEAT=ON
expect_cmake "FREESTANDING + BACKEND_LINUXKMS" false false \
    -DSLINT_FEATURE_FREESTANDING=ON -DSLINT_FEATURE_BACKEND_LINUXKMS=ON

echo "== bindings and tools: default builds ship libinput, never libseat =="
expect "slint-node (default)" false true \
    -p slint-node
expect "slint-python (default)" false true \
    -p slint-python
expect "slint-viewer + backend-linuxkms-libinput" false true \
    -p slint-viewer --features backend-linuxkms-libinput
expect "slint-lsp + backend-linuxkms-libinput" false true \
    -p slint-lsp --features backend-linuxkms-libinput

if [ "$failures" -ne 0 ]; then
    echo
    echo "$failures LinuxKMS feature resolution check(s) failed."
    exit 1
fi
echo
echo "All LinuxKMS feature resolution checks passed."
