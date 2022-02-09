#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint-ui.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

cargo publish --manifest-path internal/common/Cargo.toml
cargo publish --manifest-path internal/core-macros/Cargo.toml
cargo publish --manifest-path internal/compiler/Cargo.toml
cargo publish --manifest-path internal/core/Cargo.toml
cargo publish --manifest-path api/rs/macros/Cargo.toml
cargo publish --manifest-path internal/backends/gl/Cargo.toml --features x11
cargo publish --manifest-path api/rs/build/Cargo.toml
cargo publish --manifest-path internal/backends/qt/Cargo.toml
sleep 30
cargo publish --manifest-path internal/backends/selector/Cargo.toml --features x11
sleep 30
cargo publish --manifest-path internal/interpreter/Cargo.toml
cargo publish --manifest-path api/rs/slint/Cargo.toml
cargo publish --manifest-path tools/lsp/Cargo.toml
cargo publish --manifest-path tools/viewer/Cargo.toml
