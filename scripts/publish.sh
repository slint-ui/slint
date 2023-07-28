#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

cargo publish --manifest-path internal/common/Cargo.toml
cargo publish --manifest-path internal/core-macros/Cargo.toml
sleep 15
cargo publish --manifest-path internal/compiler/Cargo.toml
cargo publish --manifest-path internal/core/Cargo.toml
sleep 15
cargo publish --manifest-path api/rs/macros/Cargo.toml
cargo publish --manifest-path internal/renderers/skia/Cargo.toml
cargo publish --manifest-path internal/renderers/femtovg/Cargo.toml
cargo publish --manifest-path internal/backends/winit/Cargo.toml --features x11,renderer-femtovg
cargo publish --manifest-path api/rs/build/Cargo.toml
cargo publish --manifest-path internal/backends/qt/Cargo.toml
cargo publish --manifest-path internal/backends/linuxkms/Cargo.toml
sleep 30
cargo publish --manifest-path internal/backends/selector/Cargo.toml --features backend-winit-x11,renderer-femtovg
sleep 30
cargo publish --manifest-path internal/interpreter/Cargo.toml
cargo publish --manifest-path api/rs/slint/Cargo.toml
cargo publish --manifest-path tools/lsp/Cargo.toml
cargo publish --manifest-path tools/viewer/Cargo.toml
cargo publish --manifest-path tools/updater/Cargo.toml
cargo publish --manifest-path tools/tr-extractor/Cargo.toml

