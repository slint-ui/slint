#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Run cargo clippy (this script is used by CI)

export RUSTFLAGS="-D warnings"
export CARGO_INCREMENTAL=false
export CARGO_PROFILE_DEV_DEBUG=0

# The repository is split into several workspaces (root libraries/tools,
# examples, demos and tests) that all share the same target directory, so the
# common library crates are only built once across these clippy runs.

# Root workspace: libraries and tools.
# slint-node/slint-cpp/slint-python need their language bindings toolchains.
cargo clippy --locked --all-features --workspace \
    --exclude slint-node \
    --exclude slint-cpp \
    --exclude slint-python \
    -- -D warnings

# Examples workspace. The mcu/uefi members need dedicated targets, and
# bevy/servo have heavy dependencies and dedicated CI workflows.
cargo clippy --locked --all-features --workspace --manifest-path examples/Cargo.toml \
    --exclude mcu-board-support \
    --exclude mcu-embassy \
    --exclude uefi-demo \
    --exclude plotter \
    --exclude gstreamer-player \
    --exclude bevy-example \
    --exclude bevy-hosts-slint \
    --exclude bevy-hosts-slint-gpu \
    --exclude servo-example \
    -- -D warnings

# Demos workspace.
cargo clippy --locked --all-features --workspace --manifest-path demos/Cargo.toml \
    --exclude printerdemo_mcu \
    -- -D warnings

# Tests workspace. The C++/Node/Python drivers need their language toolchains.
cargo clippy --locked --all-features --workspace --manifest-path tests/Cargo.toml \
    --exclude test-driver-nodejs \
    --exclude test-driver-cpp \
    --exclude test-driver-python \
    --exclude bapp \
    -- -D warnings
