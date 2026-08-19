// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-light".into())
        .with_sdf_fonts(true)
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("main.slint", config).unwrap();
}
