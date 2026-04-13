#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
#
# Run cargo clippy (this script is used by CI)

export RUSTFLAGS="-D warnings"
export CARGO_INCREMENTAL=false
export CARGO_PROFILE_DEV_DEBUG=0

cargo clippy --locked --all-features --workspace \
    --exclude slint-node \
    --exclude test-driver-nodejs \
    --exclude test-driver-cpp \
    --exclude test-driver-python \
    --exclude mcu-board-support \
    --exclude mcu-embassy \
    --exclude printerdemo_mcu \
    --exclude uefi-demo \
    --exclude slint-cpp \
    --exclude slint-python \
    --exclude plotter \
    --exclude gstreamer-player \
    --exclude material-gallery \
    --exclude bapp \
    -- -D warnings
