#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Run cargo clippy (this script is used by CI)

set -e

# The repository is split into several workspaces that all share the same
# target directory, so the common library crates are only built once across
# these clippy runs.
# Run every workspace even when an earlier one fails, so a single CI run
# reports the errors of all of them.
status=0

# Root workspace: libraries and tools.
# slint-node/slint-cpp/slint-python need their language bindings toolchains.
cargo clippy --locked --all-features --workspace \
    --exclude slint-node \
    --exclude slint-cpp \
    --exclude slint-python \
    -- -D warnings || status=1

# Examples workspace. bevy has heavy dependencies and a dedicated CI workflow.
cargo clippy --locked --all-features --workspace --manifest-path examples/Cargo.toml \
    --exclude plotter \
    --exclude gstreamer-player \
    --exclude bevy-example \
    --exclude bevy-hosts-slint \
    --exclude bevy-hosts-slint-gpu \
    -- -D warnings || status=1

# Demos workspace.
cargo clippy --locked --all-features --workspace --manifest-path demos/Cargo.toml \
    --exclude printerdemo_mcu \
    -- -D warnings || status=1

# Tests workspace. The C++/Node/Python drivers need their language toolchains.
cargo clippy --locked --all-features --workspace --manifest-path tests/Cargo.toml \
    --exclude test-driver-nodejs \
    --exclude test-driver-cpp \
    --exclude test-driver-python \
    --exclude bapp \
    -- -D warnings || status=1

# Zed extension workspace.
cargo clippy --all-features --manifest-path editors/zed/Cargo.toml -- -D warnings || status=1

exit $status
