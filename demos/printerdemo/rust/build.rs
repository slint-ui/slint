// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    let mut config = slint_build::CompilerConfiguration::new();
    let target = std::env::var("TARGET").unwrap();
    if target.contains("android") || target.contains("wasm32") {
        config = config.with_bundled_translations("../lang");
    }
    slint_build::compile_with_config("../ui/printerdemo.slint", config).unwrap();
}
