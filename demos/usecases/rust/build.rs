// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint_build::CompilerConfiguration;
use std::env;

fn main() {
    let style = if env::var("TARGET").unwrap().contains("android") { "material" } else { "cosmic" }
        .to_string();

    slint_build::compile_with_config(
        "../ui/app.slint",
        CompilerConfiguration::new()
            .with_style(style)
            .with_bundled_translations(concat!(env!("CARGO_MANIFEST_DIR"), "/../lang/")),
    )
    .unwrap();
}
