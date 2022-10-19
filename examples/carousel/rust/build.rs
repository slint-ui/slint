// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(not(feature = "mcu"))]
fn main() {
    slint_build::compile("../ui/carousel_demo.slint").unwrap();
}

#[cfg(feature = "mcu")]
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../ui/carousel_demo.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
