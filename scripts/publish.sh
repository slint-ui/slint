#!/bin/bash -e
cargo publish --manifest-path sixtyfps_runtime/corelib_macros/Cargo.toml
cargo publish --manifest-path sixtyfps_compiler/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/corelib/Cargo.toml --features x11
cargo publish --manifest-path api/sixtyfps-rs/sixtyfps-macros/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/gl/Cargo.toml
cargo publish --manifest-path api/sixtyfps-rs/sixtyfps-build/Cargo.toml
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/qt/Cargo.toml
sleep 30
cargo publish --manifest-path sixtyfps_runtime/rendering_backends/default/Cargo.toml
sleep 30
cargo publish --manifest-path sixtyfps_runtime/interpreter/Cargo.toml
cargo publish --manifest-path api/sixtyfps-rs/Cargo.toml

