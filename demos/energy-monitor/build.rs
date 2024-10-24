// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(not(feature = "mcu-board-support"))]
fn main() {
    slint_build::compile("ui/desktop_window.slint").unwrap();
}

#[cfg(feature = "mcu-board-support")]
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("ui/mcu_window.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
