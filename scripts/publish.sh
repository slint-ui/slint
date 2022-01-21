#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@sixtyfps.io>
# SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

cargo publish --manifest-path sixtyfps_runtime/common/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/corelib_macros/Cargo.toml
cargo publish --manifest-path sixtyfps_compiler/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/corelib/Cargo.toml
cargo publish --manifest-path api/sixtyfps-rs/sixtyfps-macros/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/gl/Cargo.toml --features x11
cargo publish --manifest-path api/sixtyfps-rs/sixtyfps-build/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/qt/Cargo.toml
sleep 30
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/default/Cargo.toml --features x11
sleep 30
cargo publish --manifest-path sixtyfps_runtime/interpreter/Cargo.toml
cargo publish --manifest-path api/sixtyfps-rs/Cargo.toml
cargo publish --manifest-path tools/lsp/Cargo.toml
cargo publish --manifest-path tools/viewer/Cargo.toml
