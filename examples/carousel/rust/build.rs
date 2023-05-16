// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

#[cfg(not(feature = "mcu-board-support"))]
fn main() {
    slint_build::compile("../ui/carousel_demo.slint").unwrap();
}

#[cfg(feature = "mcu-board-support")]
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../ui/carousel_demo.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
