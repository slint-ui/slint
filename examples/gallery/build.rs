// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile_with_config(
        "gallery.slint",
        slint_build::CompilerConfiguration::new()
            .with_bundled_translations(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/")),
    )
    .unwrap();
}
