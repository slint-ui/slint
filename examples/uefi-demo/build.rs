// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile_with_config(
        "slint-project.json",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent-dark".to_owned())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer),
    )
    .unwrap();
}
