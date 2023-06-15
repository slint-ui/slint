// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile_with_config(
        "demo.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent-dark".to_owned())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer),
    )
    .unwrap();
}
